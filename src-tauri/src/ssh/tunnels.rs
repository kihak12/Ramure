//! SSH tunnels: local forwards (`-L`), remote forwards (`-R`) and SOCKS proxies (`-D`).
//!
//! The tunnels belong to a group, the stable key of a tab of the SSH module, which outlives a
//! connection: a reconnection opens a new SSH connection in the same group. A group keeps its
//! started tunnels, their listeners, and its current connection, or none. While it has none, the
//! listeners stay bound, so that no other program takes their ports, and close every new
//! connection at once (guidelines, 9.15, “Tunnels” and “A lost connection”).
//!
//! The state of the groups sits behind one mutex, which is never held across an `.await`. Every
//! change of a tunnel is said by the `tunnel-state` event, sent while the mutex is held, so that
//! the events of a tunnel arrive in the order of its changes.

use std::collections::HashMap;
use std::io;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use russh::client::{self, Handle};
use russh::{Channel, ChannelOpenFailure, ChannelStream};
use serde::Serialize;
use tauri::State;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio::task::JoinHandle;

use super::ClientHandler;
use super::socks;
use super::store::{Tunnel, TunnelKind};

/// The SSH connection a group is attached to.
pub(super) type Link = Arc<Handle<ClientHandler>>;

/// How long a target of this machine has to answer a connection of a remote forward, a SOCKS
/// client to send its request, and the server to forget a remote forward.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const CANCEL_TIMEOUT: Duration = Duration::from_secs(5);

/// Where a tunnel is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Starting,
    Listening,
    /// The group has no connection: it is being reestablished.
    Waiting,
    Stopped,
    Failed,
}

/// The last connection the other side refused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LastFailure {
    pub message: String,
    /// In milliseconds since the epoch.
    pub at: i64,
}

/// The payload of the `tunnel-state` event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelState {
    pub group: String,
    pub id: String,
    pub state: Phase,
    /// Open connections through the tunnel.
    pub connections: u32,
    /// When `failed`: why, and what to do.
    pub error: Option<String>,
    pub last_failure: Option<LastFailure>,
}

// The messages the guidelines quote (9.15).

fn in_use_here(port: u16) -> String {
    format!("Port {port} is already in use on this machine. Change the local port, or stop the program that uses it.")
}

fn in_use_by(port: u16, owner: &str) -> String {
    if owner.is_empty() {
        format!("Port {port} is already in use by the tunnel of another session. Change the local port, or stop that tunnel.")
    } else {
        format!("Port {port} is already in use by the tunnel of {owner}. Change the local port, or stop that tunnel.")
    }
}

const NEEDS_ADMINISTRATOR: &str = "Ports under 1024 need administrator rights on this machine.";

fn refused_by_server(port: u16) -> String {
    format!("The server refused to listen on port {port}. It may be in use, or its SSH settings may not allow it.")
}

/// `host:port`, with an IPv6 address in brackets.
fn address(host: &str, port: u16) -> String {
    if host.contains(':') { format!("[{host}]:{port}") } else { format!("{host}:{port}") }
}

fn nothing_on_server(host: &str, port: u16) -> String {
    format!("Last connection refused: nothing answers at {} on the server.", address(host, port))
}

fn nothing_here(host: &str, port: u16) -> String {
    format!("Last connection refused: nothing answers at {} on this machine.", address(host, port))
}

/// What a failure to listen on `port` of this machine says. `tunnel`: the owner of the tunnel of
/// Ramure that holds the port, if one does.
fn bind_message(port: u16, error: &io::Error, tunnel: Option<&str>) -> String {
    match error.kind() {
        io::ErrorKind::AddrInUse => match tunnel {
            Some(owner) => in_use_by(port, owner),
            None => in_use_here(port),
        },
        io::ErrorKind::PermissionDenied if port < 1024 => NEEDS_ADMINISTRATOR.to_string(),
        _ => format!("Port {port} could not be opened: {error}"),
    }
}

/// The address a remote forward asks the server to listen on, as OpenSSH sends it: the loopback
/// of the server, or every interface (which the server allows only if its `GatewayPorts` does).
fn remote_address(tunnel: &Tunnel) -> &'static str {
    if tunnel.bind_any { "" } else { "localhost" }
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn same(current: &Option<Link>, link: &Link) -> bool {
    current.as_ref().is_some_and(|current| Arc::ptr_eq(current, link))
}

/// A tunnel that was asked to start, and what became of it.
struct Running {
    tunnel: Tunnel,
    /// Changes with every start: the tasks of an earlier start no longer touch this one.
    serial: u64,
    phase: Phase,
    connections: u32,
    error: Option<String>,
    last_failure: Option<LastFailure>,
    /// The connection the tasks of the tunnel go through. Its tasks watch it: when it changes,
    /// their connections are cut; when it is dropped with the tunnel, they end.
    link: watch::Sender<Option<Link>>,
    /// The tasks that accept on this machine (local forward, SOCKS): one per address.
    listeners: Vec<JoinHandle<()>>,
}

impl Running {
    fn state(&self, group: &str, id: &str) -> TunnelState {
        TunnelState {
            group: group.to_string(),
            id: id.to_string(),
            state: self.phase,
            connections: self.connections,
            error: self.error.clone(),
            last_failure: self.last_failure.clone(),
        }
    }

    /// True while the tunnel holds a port of this machine.
    fn holds_port(&self, port: u16) -> bool {
        self.tunnel.kind != TunnelKind::Remote && self.tunnel.listen_port == port && !self.listeners.is_empty()
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        // A tunnel that is forgotten frees its ports. `shut` does it first, and waits for it.
        for task in &self.listeners {
            task.abort();
        }
    }
}

#[derive(Default)]
struct Group {
    /// The display name of the host, for the message of a port that another group holds.
    owner: String,
    link: Option<Link>,
    tunnels: HashMap<String, Running>,
}

/// The tunnel `id` of `group`, as one start made it.
#[derive(Clone, Debug)]
struct Key {
    group: String,
    id: String,
    serial: u64,
}

struct Inner {
    groups: Mutex<HashMap<String, Group>>,
    serial: AtomicU64,
    emit: Box<dyn Fn(&TunnelState) + Send + Sync>,
}

/// The tunnels of every group: managed by Tauri, and held by the client handler of each
/// connection of a session, which routes the connections of its remote forwards here.
#[derive(Clone)]
pub struct Tunnels(Arc<Inner>);

impl Tunnels {
    /// `emit` says every change of a tunnel: the `tunnel-state` event.
    pub fn new(emit: impl Fn(&TunnelState) + Send + Sync + 'static) -> Self {
        Tunnels(Arc::new(Inner { groups: Mutex::new(HashMap::new()), serial: AtomicU64::new(0), emit: Box::new(emit) }))
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, Group>> {
        // The state stays usable after a panic: each change leaves it whole.
        self.0.groups.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn emit(&self, group: &str, id: &str, running: &Running) {
        (self.0.emit)(&running.state(group, id));
    }

    /// Applies `change` to the tunnel of `key` if it is still that start, and says the new state
    /// if `change` returns true. False if the tunnel was stopped or started again meanwhile.
    fn update(&self, key: &Key, change: impl FnOnce(&mut Running) -> bool) -> bool {
        let mut groups = self.lock();
        let Some(running) = groups
            .get_mut(&key.group)
            .and_then(|group| group.tunnels.get_mut(&key.id))
            .filter(|running| running.serial == key.serial)
        else {
            return false;
        };
        if change(running) {
            (self.0.emit)(&running.state(&key.group, &key.id));
        }
        true
    }

    /// Records a connection the other side refused. The tunnel keeps listening: the next one may
    /// get through.
    fn refused(&self, key: &Key, message: String) {
        self.update(key, |running| {
            running.last_failure = Some(LastFailure { message, at: now() });
            true
        });
    }

    /// The owner of the group whose tunnel, other than `key`'s, holds `port` of this machine.
    fn holder(&self, key: &Key, port: u16) -> Option<String> {
        self.lock().iter().find_map(|(name, group)| {
            group
                .tunnels
                .iter()
                .any(|(id, running)| !(*name == key.group && *id == key.id) && running.holds_port(port))
                .then(|| group.owner.clone())
        })
    }

    /// Attaches the connection of a session to its group, once the session is ready: the remote
    /// forwards ask the server again, and the other tunnels listen.
    pub(super) fn attach(&self, group: &str, link: &Link) {
        let mut requests = Vec::new();
        {
            let mut groups = self.lock();
            let entry = groups.entry(group.to_string()).or_default();
            entry.link = Some(link.clone());
            for (id, running) in &mut entry.tunnels {
                running.link.send_replace(Some(link.clone()));
                match running.tunnel.kind {
                    // Refused by the previous server or not, it asks this one.
                    TunnelKind::Remote => {
                        running.phase = Phase::Starting;
                        running.error = None;
                        requests.push(Key { group: group.to_string(), id: id.clone(), serial: running.serial });
                    }
                    // A port that could not be opened stays failed: the connection changes nothing.
                    _ if running.phase == Phase::Waiting => running.phase = Phase::Listening,
                    _ => continue,
                }
                self.emit(group, id, running);
            }
        }
        for key in requests {
            tokio::spawn(self.clone().forward(key, link.clone()));
        }
    }

    /// Detaches the connection of a session whose pump has ended, for any reason. The listeners
    /// stay bound and the open connections are cut: the tunnels wait for the next connection.
    pub(super) fn detach(&self, group: &str, link: &Link) {
        let mut groups = self.lock();
        let Some(entry) = groups.get_mut(group).filter(|entry| same(&entry.link, link)) else {
            // Forgotten already, or another connection has taken its place.
            return;
        };
        entry.link = None;
        for (id, running) in &mut entry.tunnels {
            running.link.send_replace(None);
            if matches!(running.phase, Phase::Listening | Phase::Starting) {
                running.phase = Phase::Waiting;
                self.emit(group, id, running);
            }
        }
    }

    /// Starts `tunnel` in `group`, or starts it again if it runs. A tunnel that cannot listen is
    /// not an error of the call: it goes to `failed`, with its message.
    pub async fn start(&self, group: &str, owner: &str, tunnel: Tunnel) -> Result<(), String> {
        check(group, &tunnel)?;
        let key = Key {
            group: group.to_string(),
            id: tunnel.id.clone(),
            serial: self.0.serial.fetch_add(1, Ordering::Relaxed) + 1,
        };
        let (previous, link) = {
            let mut groups = self.lock();
            let entry = groups.entry(group.to_string()).or_default();
            entry.owner = owner.to_string();
            let running = Running {
                tunnel: tunnel.clone(),
                serial: key.serial,
                phase: Phase::Starting,
                connections: 0,
                error: None,
                last_failure: None,
                link: watch::Sender::new(entry.link.clone()),
                listeners: Vec::new(),
            };
            self.emit(group, &key.id, &running);
            (entry.tunnels.insert(key.id.clone(), running), entry.link.clone())
        };
        // The previous start gives its port back first.
        if let Some(previous) = previous {
            shut(previous).await;
        }

        if tunnel.kind == TunnelKind::Remote {
            match link {
                Some(link) => self.clone().forward(key, link).await,
                None => {
                    self.update(&key, |running| {
                        running.phase = Phase::Waiting;
                        true
                    });
                }
            }
            return Ok(());
        }

        let port = tunnel.listen_port;
        let bound = match self.holder(&key, port) {
            Some(owner) => Err(in_use_by(port, &owner)),
            None => match bind(port, tunnel.bind_any).await {
                Ok(listeners) => Ok(listeners),
                Err(e) => Err(bind_message(port, &e, self.holder(&key, port).as_deref())),
            },
        };
        match bound {
            Err(message) => {
                self.update(&key, |running| {
                    running.phase = Phase::Failed;
                    running.error = Some(message);
                    true
                });
            }
            Ok(listeners) => {
                // If the tunnel was stopped or started again meanwhile, the listeners are dropped
                // unused, which frees the port.
                self.update(&key, |running| {
                    for listener in listeners {
                        let accepting =
                            self.clone().accept(key.clone(), running.tunnel.clone(), listener, running.link.subscribe());
                        running.listeners.push(tokio::spawn(accepting));
                    }
                    running.phase = if running.link.borrow().is_some() { Phase::Listening } else { Phase::Waiting };
                    true
                });
            }
        }
        Ok(())
    }

    /// Stops the tunnel `id` of `group` and frees its port; the server forgets a remote forward.
    pub async fn stop(&self, group: &str, id: &str) {
        let removed = {
            let mut groups = self.lock();
            let removed = groups.get_mut(group).and_then(|entry| entry.tunnels.remove(id));
            if let Some(running) = &removed {
                (self.0.emit)(&TunnelState { state: Phase::Stopped, connections: 0, ..running.state(group, id) });
            }
            removed
        };
        if let Some(running) = removed {
            shut(running).await;
        }
    }

    /// Stops every tunnel of `group`, frees their ports, and forgets the group.
    pub async fn close(&self, group: &str) {
        let removed = {
            let mut groups = self.lock();
            let removed = groups.remove(group).map(|entry| entry.tunnels).unwrap_or_default();
            for (id, running) in &removed {
                (self.0.emit)(&TunnelState { state: Phase::Stopped, connections: 0, ..running.state(group, id) });
            }
            removed
        };
        for (_, running) in removed {
            shut(running).await;
        }
    }

    /// Asks the server of `link` to listen for the remote forward of `key`.
    async fn forward(self, key: Key, link: Link) {
        let Some(tunnel) = self.tunnel(&key) else { return };
        let address = remote_address(&tunnel);
        let port = tunnel.listen_port;
        let result = link.tcpip_forward(address, u32::from(port)).await;
        let current = self.update(&key, |running| {
            if !same(&running.link.borrow(), &link) {
                // Another connection has taken its place meanwhile: it asks for itself.
                return false;
            }
            match &result {
                Ok(_) => running.phase = Phase::Listening,
                Err(russh::Error::RequestDenied) => {
                    running.phase = Phase::Failed;
                    running.error = Some(refused_by_server(port));
                }
                // The connection is gone: detaching says so.
                Err(_) => return false,
            }
            true
        });
        if result.is_ok() && !current && !self.forwards(&key.group, port, &link) {
            // Stopped while the server was answering: it forgets the port again.
            let _ = tokio::time::timeout(CANCEL_TIMEOUT, link.cancel_tcpip_forward(address, u32::from(port))).await;
        }
    }

    /// The tunnel of `key`, if it is still that start.
    fn tunnel(&self, key: &Key) -> Option<Tunnel> {
        let groups = self.lock();
        let running = groups.get(&key.group)?.tunnels.get(&key.id)?;
        (running.serial == key.serial).then(|| running.tunnel.clone())
    }

    /// True if a remote forward of `group` on `port` of the server goes through `link`.
    fn forwards(&self, group: &str, port: u16, link: &Link) -> bool {
        self.lock().get(group).is_some_and(|entry| {
            entry.tunnels.values().any(|running| {
                running.tunnel.kind == TunnelKind::Remote
                    && running.tunnel.listen_port == port
                    && matches!(running.phase, Phase::Starting | Phase::Listening)
                    && same(&running.link.borrow(), link)
            })
        })
    }

    /// Accepts the connections of a local forward or a SOCKS proxy on one of its addresses.
    async fn accept(self, key: Key, tunnel: Tunnel, listener: TcpListener, link: watch::Receiver<Option<Link>>) {
        loop {
            let (stream, peer) = match listener.accept().await {
                Ok(accepted) => accepted,
                Err(_) => {
                    // Out of file descriptors, for one: the next accept may work.
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
            };
            let Some(conn) = link.borrow().clone() else {
                // Waiting for the connection: closed at once, so that the program sees a refusal
                // rather than a hang.
                let _ = stream.set_zero_linger();
                continue;
            };
            let _ = stream.set_nodelay(true);
            let tunnels = self.clone();
            let (key, link, tunnel) = (key.clone(), link.clone(), tunnel.clone());
            match tunnel.kind {
                TunnelKind::Socks => tokio::spawn(tunnels.socks(key, stream, peer, conn, link)),
                _ => tokio::spawn(tunnels.local(key, tunnel, stream, peer, conn, link)),
            };
        }
    }

    /// One connection of a local forward: a `direct-tcpip` channel to the target, as the server
    /// sees it.
    async fn local(
        self,
        key: Key,
        tunnel: Tunnel,
        stream: TcpStream,
        peer: SocketAddr,
        conn: Link,
        link: watch::Receiver<Option<Link>>,
    ) {
        let (host, port) = (tunnel.target_host.as_str(), tunnel.target_port);
        match conn.channel_open_direct_tcpip(host, u32::from(port), peer.ip().to_string(), u32::from(peer.port())).await {
            Ok(channel) => self.carry(&key, stream, channel.into_stream(), &conn, link).await,
            Err(russh::Error::ChannelOpenFailure(_)) => self.refused(&key, nothing_on_server(host, port)),
            // The connection is gone: the stream closes.
            Err(_) => {}
        }
    }

    /// One connection of a SOCKS proxy: its request, then a `direct-tcpip` channel to the target
    /// it names.
    async fn socks(
        self,
        key: Key,
        mut stream: TcpStream,
        peer: SocketAddr,
        conn: Link,
        link: watch::Receiver<Option<Link>>,
    ) {
        let Ok(Ok(target)) = tokio::time::timeout(CONNECT_TIMEOUT, socks::handshake(&mut stream)).await else {
            return;
        };
        let opened = conn
            .channel_open_direct_tcpip(target.host.as_str(), u32::from(target.port), peer.ip().to_string(), u32::from(peer.port()))
            .await;
        match opened {
            Ok(channel) => {
                let channel = channel.into_stream();
                if stream.write_all(&socks::reply(socks::SUCCEEDED)).await.is_ok() {
                    self.carry(&key, stream, channel, &conn, link).await;
                }
            }
            Err(russh::Error::ChannelOpenFailure(_)) => {
                let _ = stream.write_all(&socks::reply(socks::CONNECTION_REFUSED)).await;
                self.refused(&key, nothing_on_server(&target.host, target.port));
            }
            Err(_) => {
                let _ = stream.write_all(&socks::reply(socks::GENERAL_FAILURE)).await;
            }
        }
    }

    /// A connection the server forwards for a remote forward of `group` that listens on `port`
    /// of the server: it goes to the target, as this machine sees it. Called by the client
    /// handler, which must not wait: the connection to the target is made in a task of its own.
    pub(super) fn forwarded(&self, group: &str, port: u32, channel: Channel<client::Msg>, reply: client::ChannelOpenHandle) {
        let found = {
            let groups = self.lock();
            groups.get(group).and_then(|entry| {
                entry.tunnels.iter().find_map(|(id, running)| {
                    let wanted = running.tunnel.kind == TunnelKind::Remote
                        && u32::from(running.tunnel.listen_port) == port
                        && matches!(running.phase, Phase::Starting | Phase::Listening);
                    let conn = running.link.borrow().clone().filter(|_| wanted)?;
                    let key = Key { group: group.to_string(), id: id.clone(), serial: running.serial };
                    Some((key, running.tunnel.clone(), conn, running.link.subscribe()))
                })
            })
        };
        let Some((key, tunnel, conn, link)) = found else {
            // No tunnel of this session listens there: dropping the reply refuses the channel.
            drop(reply);
            return;
        };
        let tunnels = self.clone();
        tokio::spawn(async move {
            let (host, port) = (tunnel.target_host.as_str(), tunnel.target_port);
            match tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect((host, port))).await {
                Ok(Ok(stream)) => {
                    let _ = stream.set_nodelay(true);
                    reply.accept().await;
                    tunnels.carry(&key, stream, channel.into_stream(), &conn, link).await;
                }
                _ => {
                    reply.reject(ChannelOpenFailure::ConnectFailed).await;
                    tunnels.refused(&key, nothing_here(host, port));
                }
            }
        });
    }

    /// Carries a connection both ways until both sides have ended, or until the connection of
    /// the group changes or the tunnel stops: it is cut then. Counted while it lasts.
    async fn carry(
        &self,
        key: &Key,
        mut stream: TcpStream,
        mut channel: ChannelStream<client::Msg>,
        conn: &Link,
        mut link: watch::Receiver<Option<Link>>,
    ) {
        let Some(_counted) = Counted::new(self, key) else { return };
        tokio::select! {
            _ = tokio::io::copy_bidirectional(&mut stream, &mut channel) => {}
            _ = link.wait_for(|current| !same(current, conn)) => {}
        }
    }
}

/// An open connection of a tunnel: counted from its creation to its drop, whatever ends it.
struct Counted {
    tunnels: Tunnels,
    key: Key,
}

impl Counted {
    fn new(tunnels: &Tunnels, key: &Key) -> Option<Self> {
        tunnels
            .update(key, |running| {
                running.connections += 1;
                true
            })
            .then(|| Counted { tunnels: tunnels.clone(), key: key.clone() })
    }
}

impl Drop for Counted {
    fn drop(&mut self) {
        self.tunnels.update(&self.key, |running| {
            running.connections = running.connections.saturating_sub(1);
            true
        });
    }
}

/// A malformed call: the interface checks these before it calls.
fn check(group: &str, tunnel: &Tunnel) -> Result<(), String> {
    if group.is_empty() || tunnel.id.is_empty() {
        return Err("This tunnel belongs to no session, or has no identifier: it was not started. Remove it, then add it again.".into());
    }
    if tunnel.listen_port == 0 {
        return Err("Choose a port from 1 to 65535 for the tunnel to listen on.".into());
    }
    if tunnel.kind != TunnelKind::Socks {
        if tunnel.target_host.trim().is_empty() {
            return Err("Enter the host the tunnel reaches.".into());
        }
        if tunnel.target_port == 0 {
            return Err("Choose a port from 1 to 65535 for the tunnel to reach.".into());
        }
    }
    Ok(())
}

/// Lets go of a tunnel: its open connections are cut, its listeners end, which frees its port
/// before this returns, and the server forgets a remote forward.
async fn shut(mut running: Running) {
    let link = running.link.borrow().clone();
    let forwarded = running.tunnel.kind == TunnelKind::Remote && running.phase == Phase::Listening;
    // The tasks of its connections end once the sender is gone, when `running` is dropped.
    running.link.send_replace(None);
    let listeners = std::mem::take(&mut running.listeners);
    for task in &listeners {
        task.abort();
    }
    for task in listeners {
        let _ = task.await;
    }
    if let (true, Some(link)) = (forwarded, link) {
        let port = u32::from(running.tunnel.listen_port);
        let address = remote_address(&running.tunnel);
        let _ = tokio::time::timeout(CANCEL_TIMEOUT, link.cancel_tcpip_forward(address, port)).await;
    }
}

/// Opens `port` of this machine: on `localhost` in IPv4 and in IPv6, or on every interface.
/// IPv6 is left out where the system does not have it.
async fn bind(port: u16, bind_any: bool) -> io::Result<Vec<TcpListener>> {
    if bind_any && cfg!(not(windows)) {
        // On Linux and macOS, `[::]` takes IPv4 too, and `0.0.0.0` is then refused: it is only
        // needed where the system keeps IPv6 apart.
        match TcpListener::bind((Ipv6Addr::UNSPECIFIED, port)).await {
            Ok(v6) => {
                let mut listeners = vec![v6];
                if let Ok(v4) = TcpListener::bind((Ipv4Addr::UNSPECIFIED, port)).await {
                    listeners.push(v4);
                }
                return Ok(listeners);
            }
            Err(e) if matches!(e.kind(), io::ErrorKind::AddrInUse | io::ErrorKind::PermissionDenied) => return Err(e),
            Err(_) => return Ok(vec![TcpListener::bind((Ipv4Addr::UNSPECIFIED, port)).await?]),
        }
    }
    let (v4, v6) = if bind_any {
        (Ipv4Addr::UNSPECIFIED, Ipv6Addr::UNSPECIFIED)
    } else {
        (Ipv4Addr::LOCALHOST, Ipv6Addr::LOCALHOST)
    };
    let mut listeners = vec![TcpListener::bind((v4, port)).await?];
    match TcpListener::bind((v6, port)).await {
        Ok(listener) => listeners.push(listener),
        // Another program listens there in IPv6: `localhost` would reach it.
        Err(e) if e.kind() == io::ErrorKind::AddrInUse => return Err(e),
        Err(_) => {}
    }
    Ok(listeners)
}

/// Starts a tunnel in a group, or starts it again if one with the same `id` runs. `owner`: the
/// display name of the host, for the message of a port that this tunnel holds when another
/// group asks for it.
#[tauri::command]
pub async fn tunnel_start(tunnels: State<'_, Tunnels>, group: String, owner: String, tunnel: Tunnel) -> Result<(), String> {
    tunnels.start(&group, &owner, tunnel).await
}

/// Stops one tunnel and frees its port.
#[tauri::command]
pub async fn tunnel_stop(tunnels: State<'_, Tunnels>, group: String, id: String) -> Result<(), String> {
    tunnels.stop(&group, &id).await;
    Ok(())
}

/// Stops every tunnel of a group, frees their ports, and forgets the group: the session is over
/// for good.
#[tauri::command]
pub async fn tunnels_close(tunnels: State<'_, Tunnels>, group: String) -> Result<(), String> {
    tunnels.close(&group).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use russh::keys::{Algorithm, HashAlg, PrivateKey};
    use russh::server;
    use tokio::io::AsyncReadExt;
    use tokio::sync::mpsc;

    const WAIT: Duration = Duration::from_secs(5);

    /// The `tunnel-state` events, in order.
    struct Events(mpsc::UnboundedReceiver<TunnelState>);

    impl Events {
        /// The next state of `id` that `wanted` accepts; the states before it are passed over.
        async fn until(&mut self, id: &str, wanted: impl Fn(&TunnelState) -> bool) -> TunnelState {
            let next = async {
                loop {
                    let state = self.0.recv().await.expect("the events ended");
                    if state.id == id && wanted(&state) {
                        return state;
                    }
                }
            };
            tokio::time::timeout(WAIT, next).await.unwrap_or_else(|_| panic!("{id} never reached the state awaited"))
        }
    }

    fn manager() -> (Tunnels, Events) {
        let (tx, rx) = mpsc::unbounded_channel();
        let tunnels = Tunnels::new(move |state| {
            let _ = tx.send(state.clone());
        });
        (tunnels, Events(rx))
    }

    fn local(id: &str, listen_port: u16, target_port: u16) -> Tunnel {
        Tunnel {
            id: id.into(),
            kind: TunnelKind::Local,
            listen_port,
            target_host: "127.0.0.1".into(),
            target_port,
            ..Tunnel::default()
        }
    }

    fn remote(id: &str, listen_port: u16, target_port: u16) -> Tunnel {
        Tunnel { kind: TunnelKind::Remote, ..local(id, listen_port, target_port) }
    }

    fn socks_proxy(id: &str, listen_port: u16) -> Tunnel {
        Tunnel { id: id.into(), kind: TunnelKind::Socks, listen_port, ..Tunnel::default() }
    }

    /// A port nothing listens on, for now.
    fn free_port() -> u16 {
        std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
    }

    fn port_is_free(port: u16) -> bool {
        std::net::TcpListener::bind(("127.0.0.1", port)).is_ok()
    }

    /// A service of this machine that sends back what it receives.
    async fn echo() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let (mut from, mut to) = socket.split();
                    let _ = tokio::io::copy(&mut from, &mut to).await;
                });
            }
        });
        port
    }

    async fn exchange(stream: &mut TcpStream, word: &[u8]) {
        stream.write_all(word).await.unwrap();
        let mut back = vec![0; word.len()];
        tokio::time::timeout(WAIT, stream.read_exact(&mut back)).await.expect("no answer").unwrap();
        assert_eq!(back, word);
    }

    /// True once the other end has closed the stream, at once rather than after a hang.
    async fn closed(stream: &mut TcpStream) -> bool {
        let mut byte = [0; 1];
        matches!(tokio::time::timeout(WAIT, stream.read(&mut byte)).await, Ok(Ok(0) | Err(_)))
    }

    async fn connect_to(port: u16) -> TcpStream {
        TcpStream::connect(("127.0.0.1", port)).await.unwrap()
    }

    /// True if a new connection to `port` is refused at once: the reset of a waiting tunnel can
    /// even reach the program before its `connect` returns.
    async fn refused_at_once(port: u16) -> bool {
        match TcpStream::connect(("127.0.0.1", port)).await {
            Ok(mut stream) => closed(&mut stream).await,
            Err(e) => e.kind() == io::ErrorKind::ConnectionReset,
        }
    }

    /// An SSH server in the test: anyone gets in, `direct-tcpip` channels go where they ask, and
    /// a remote forward listens on 127.0.0.1, except on the port `refused`.
    #[derive(Clone, Default)]
    struct TestServer {
        refused: u16,
        cancels: Arc<Mutex<Vec<u32>>>,
    }

    impl server::Handler for TestServer {
        type Error = russh::Error;

        async fn auth_none(&mut self, _user: &str) -> Result<server::Auth, Self::Error> {
            Ok(server::Auth::Accept)
        }

        async fn channel_open_direct_tcpip(
            &mut self,
            channel: Channel<server::Msg>,
            host: &str,
            port: u32,
            _originator_address: &str,
            _originator_port: u32,
            reply: server::ChannelOpenHandle,
            _session: &mut server::Session,
        ) -> Result<(), Self::Error> {
            match TcpStream::connect((host, port as u16)).await {
                Ok(mut target) => {
                    reply.accept().await;
                    tokio::spawn(async move {
                        let mut channel = channel.into_stream();
                        let _ = tokio::io::copy_bidirectional(&mut target, &mut channel).await;
                    });
                }
                Err(_) => reply.reject(ChannelOpenFailure::ConnectFailed).await,
            }
            Ok(())
        }

        async fn tcpip_forward(
            &mut self,
            address: &str,
            port: &mut u32,
            session: &mut server::Session,
        ) -> Result<bool, Self::Error> {
            let wanted = *port as u16;
            if wanted == self.refused {
                return Ok(false);
            }
            let Ok(listener) = TcpListener::bind(("127.0.0.1", wanted)).await else { return Ok(false) };
            let (handle, address) = (session.handle(), address.to_string());
            tokio::spawn(async move {
                while let Ok((mut socket, peer)) = listener.accept().await {
                    let (handle, address) = (handle.clone(), address.clone());
                    tokio::spawn(async move {
                        let originator = (peer.ip().to_string(), u32::from(peer.port()));
                        if let Ok(channel) =
                            handle.channel_open_forwarded_tcpip(address, u32::from(wanted), originator.0, originator.1).await
                        {
                            let mut channel = channel.into_stream();
                            let _ = tokio::io::copy_bidirectional(&mut socket, &mut channel).await;
                        }
                    });
                }
            });
            Ok(true)
        }

        async fn cancel_tcpip_forward(
            &mut self,
            _address: &str,
            port: u32,
            _session: &mut server::Session,
        ) -> Result<bool, Self::Error> {
            self.cancels.lock().unwrap().push(port);
            Ok(true)
        }
    }

    /// Opens an SSH connection to a new `server`, for the group `group`, as `ssh_connect` does.
    async fn connect(tunnels: &Tunnels, group: &str, server: TestServer) -> Link {
        let key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).unwrap();
        let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
        let config = Arc::new(server::Config { keys: vec![key], ..Default::default() });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let session = server::run_stream(config, socket, server).await.unwrap();
            let _ = session.await;
        });
        let handler = ClientHandler {
            host: "127.0.0.1".into(),
            port: address.port(),
            accepted: Some(fingerprint),
            remember: false,
            verdict: Arc::new(Mutex::new(None)),
            forwards: Some((tunnels.clone(), group.into())),
        };
        let mut handle = client::connect(Arc::new(client::Config::default()), address, handler).await.unwrap();
        assert!(handle.authenticate_none("tester").await.unwrap().success());
        Arc::new(handle)
    }

    #[test]
    fn the_messages_say_what_happened_and_what_to_do() {
        let in_use = io::Error::from(io::ErrorKind::AddrInUse);
        assert_eq!(
            bind_message(5433, &in_use, None),
            "Port 5433 is already in use on this machine. Change the local port, or stop the program that uses it."
        );
        assert_eq!(
            bind_message(5433, &in_use, Some("web-staging")),
            "Port 5433 is already in use by the tunnel of web-staging. Change the local port, or stop that tunnel."
        );
        let denied = io::Error::from(io::ErrorKind::PermissionDenied);
        assert_eq!(bind_message(80, &denied, None), "Ports under 1024 need administrator rights on this machine.");
        // Any other refusal is said in the words of the system.
        let other = io::Error::new(io::ErrorKind::AddrNotAvailable, "Cannot assign requested address");
        assert_eq!(bind_message(5433, &other, None), "Port 5433 could not be opened: Cannot assign requested address");
        assert_eq!(bind_message(5433, &denied, None), "Port 5433 could not be opened: permission denied");

        assert_eq!(
            refused_by_server(8080),
            "The server refused to listen on port 8080. It may be in use, or its SSH settings may not allow it."
        );
        assert_eq!(
            nothing_on_server("localhost", 5432),
            "Last connection refused: nothing answers at localhost:5432 on the server."
        );
        assert_eq!(nothing_here("localhost", 3000), "Last connection refused: nothing answers at localhost:3000 on this machine.");
        assert_eq!(nothing_on_server("::1", 5432), "Last connection refused: nothing answers at [::1]:5432 on the server.");
    }

    #[test]
    fn a_malformed_call_is_an_error() {
        assert!(check("g", &local("t", 5433, 5432)).is_ok());
        assert!(check("g", &socks_proxy("t", 1080)).is_ok());
        assert!(check("", &local("t", 5433, 5432)).is_err());
        assert!(check("g", &local("", 5433, 5432)).is_err());
        assert!(check("g", &local("t", 0, 5432)).is_err());
        assert!(check("g", &local("t", 5433, 0)).is_err());
        assert!(check("g", &Tunnel { target_host: " ".into(), ..remote("t", 8080, 3000) }).is_err());
    }

    #[test]
    fn the_event_has_the_shape_of_the_contract() {
        let state = TunnelState {
            group: "g".into(),
            id: "t".into(),
            state: Phase::Waiting,
            connections: 2,
            error: None,
            last_failure: Some(LastFailure { message: "m".into(), at: 1 }),
        };
        assert_eq!(
            serde_json::to_value(&state).unwrap(),
            serde_json::json!({
                "group": "g", "id": "t", "state": "waiting", "connections": 2, "error": null,
                "lastFailure": { "message": "m", "at": 1 },
            })
        );
    }

    #[tokio::test]
    async fn a_port_held_by_a_program_or_by_another_tunnel_fails_the_tunnel() {
        let (tunnels, mut events) = manager();

        let program = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let taken = program.local_addr().unwrap().port();
        tunnels.start("a", "web-prod", local("t1", taken, 5432)).await.unwrap();
        let failed = events.until("t1", |s| s.state == Phase::Failed).await;
        assert_eq!(failed.error, Some(in_use_here(taken)));

        // Without a connection yet, a tunnel holds its port and waits.
        let port = free_port();
        tunnels.start("b", "web-staging", local("t2", port, 5432)).await.unwrap();
        assert_eq!(events.until("t2", |s| s.state != Phase::Starting).await.state, Phase::Waiting);
        assert!(refused_at_once(port).await, "a waiting tunnel closes a new connection at once");

        tunnels.start("a", "web-prod", local("t3", port, 5432)).await.unwrap();
        let failed = events.until("t3", |s| s.state == Phase::Failed).await;
        assert_eq!(
            failed.error.as_deref(),
            Some(format!("Port {port} is already in use by the tunnel of web-staging. Change the local port, or stop that tunnel.").as_str())
        );
        // Even when the other one listens on every interface.
        tunnels.start("b", "web-staging", Tunnel { bind_any: true, ..local("t2", port, 5432) }).await.unwrap();
        assert_eq!(events.until("t2", |s| s.state != Phase::Starting).await.state, Phase::Waiting);
        tunnels.start("a", "web-prod", socks_proxy("t3", port)).await.unwrap();
        assert_eq!(events.until("t3", |s| s.state == Phase::Failed).await.error, Some(in_use_by(port, "web-staging")));

        // Once that tunnel stops, its port is free: “Retry” works.
        tunnels.stop("b", "t2").await;
        assert_eq!(events.until("t2", |_| true).await.state, Phase::Stopped);
        tunnels.start("a", "web-prod", local("t3", port, 5432)).await.unwrap();
        assert_eq!(events.until("t3", |s| s.state != Phase::Starting).await.state, Phase::Waiting);

        tunnels.close("a").await;
        assert_eq!(events.until("t3", |_| true).await.state, Phase::Stopped);
        assert!(port_is_free(port));
        // Stopping what does not run does nothing.
        tunnels.stop("a", "t3").await;
        tunnels.close("nothing").await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_local_forward_goes_through_the_server_and_waits_for_a_lost_connection() {
        let (tunnels, mut events) = manager();
        let link = connect(&tunnels, "g", TestServer::default()).await;
        tunnels.attach("g", &link);
        let target = echo().await;
        let port = free_port();
        tunnels.start("g", "vps", local("db", port, target)).await.unwrap();
        assert_eq!(events.until("db", |s| s.state != Phase::Starting).await.state, Phase::Listening);

        let mut client = connect_to(port).await;
        exchange(&mut client, b"ping").await;
        assert_eq!(events.until("db", |s| s.connections == 1).await.state, Phase::Listening);
        drop(client);
        events.until("db", |s| s.connections == 0).await;

        // Nothing answers at the target: the client is closed, and the tunnel keeps listening.
        let nothing = free_port();
        let dead = free_port();
        tunnels.start("g", "vps", local("dead", dead, nothing)).await.unwrap();
        events.until("dead", |s| s.state == Phase::Listening).await;
        let mut client = connect_to(dead).await;
        assert!(closed(&mut client).await);
        let state = events.until("dead", |s| s.last_failure.is_some()).await;
        assert_eq!(state.state, Phase::Listening);
        assert_eq!(
            state.last_failure.unwrap().message,
            format!("Last connection refused: nothing answers at 127.0.0.1:{nothing} on the server.")
        );

        // The connection is lost: the open connections are cut, the port stays taken, and a new
        // connection is closed at once.
        let mut open = connect_to(port).await;
        exchange(&mut open, b"before").await;
        events.until("db", |s| s.connections == 1).await;
        tunnels.detach("g", &link);
        events.until("db", |s| s.state == Phase::Waiting).await;
        assert!(closed(&mut open).await);
        assert_eq!(events.until("db", |s| s.connections == 0).await.state, Phase::Waiting);
        assert!(!port_is_free(port));
        assert!(refused_at_once(port).await);
        // A detach of a connection that is not the group's changes nothing.
        tunnels.detach("g", &link);

        // The next connection: the tunnels listen again.
        let again = connect(&tunnels, "g", TestServer::default()).await;
        tunnels.attach("g", &again);
        assert_eq!(events.until("db", |s| s.state == Phase::Listening).await.connections, 0);
        let mut client = connect_to(port).await;
        exchange(&mut client, b"after").await;
        events.until("db", |s| s.connections == 1).await;

        // Stopping cuts the open connections and frees the port at once.
        tunnels.stop("g", "db").await;
        assert_eq!(events.until("db", |_| true).await.state, Phase::Stopped);
        assert!(closed(&mut client).await);
        assert!(port_is_free(port));
        tunnels.close("g").await;
        assert!(port_is_free(dead));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_remote_forward_brings_the_servers_connections_to_this_machine() {
        let (tunnels, mut events) = manager();
        let busy = free_port();
        let server = TestServer { refused: busy, ..TestServer::default() };
        let cancels = server.cancels.clone();
        let link = connect(&tunnels, "g", server).await;
        tunnels.attach("g", &link);
        let target = echo().await;

        let port = free_port();
        tunnels.start("g", "vps", remote("web", port, target)).await.unwrap();
        assert_eq!(events.until("web", |s| s.state != Phase::Starting).await.state, Phase::Listening);
        let mut client = connect_to(port).await;
        exchange(&mut client, b"ping").await;
        events.until("web", |s| s.connections == 1).await;
        drop(client);
        events.until("web", |s| s.connections == 0).await;

        // The server refuses to listen.
        tunnels.start("g", "vps", remote("busy", busy, target)).await.unwrap();
        let failed = events.until("busy", |s| s.state != Phase::Starting).await;
        assert_eq!(failed.state, Phase::Failed);
        assert_eq!(
            failed.error.as_deref(),
            Some(format!("The server refused to listen on port {busy}. It may be in use, or its SSH settings may not allow it.").as_str())
        );

        // Nothing answers at the target on this machine.
        let nothing = free_port();
        let dead = free_port();
        tunnels.start("g", "vps", remote("dead", dead, nothing)).await.unwrap();
        events.until("dead", |s| s.state == Phase::Listening).await;
        let mut client = connect_to(dead).await;
        assert!(closed(&mut client).await);
        let state = events.until("dead", |s| s.last_failure.is_some()).await;
        assert_eq!(state.state, Phase::Listening);
        assert_eq!(
            state.last_failure.unwrap().message,
            format!("Last connection refused: nothing answers at 127.0.0.1:{nothing} on this machine.")
        );

        // Stopping asks the server to forget the port.
        tunnels.stop("g", "web").await;
        assert_eq!(events.until("web", |_| true).await.state, Phase::Stopped);
        assert_eq!(*cancels.lock().unwrap(), vec![u32::from(port)]);

        // A lost connection, then a new one: the remote forwards ask the new server, a forward
        // the previous server refused included.
        tunnels.detach("g", &link);
        events.until("dead", |s| s.state == Phase::Waiting).await;
        let again = connect(&tunnels, "g", TestServer::default()).await;
        tunnels.attach("g", &again);
        let back = events.until("busy", |s| s.state == Phase::Listening).await;
        assert_eq!(back.error, None);
        let mut client = connect_to(busy).await;
        exchange(&mut client, b"again").await;

        tunnels.close("g").await;
        events.until("busy", |s| s.state == Phase::Stopped).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_socks_proxy_goes_out_through_the_server() {
        let (tunnels, mut events) = manager();
        let link = connect(&tunnels, "g", TestServer::default()).await;
        tunnels.attach("g", &link);
        let target = echo().await;
        let port = free_port();
        tunnels.start("g", "vps", socks_proxy("proxy", port)).await.unwrap();
        events.until("proxy", |s| s.state == Phase::Listening).await;

        /// Greets the proxy and asks for `request`; returns the reply code.
        async fn ask(port: u16, request: &[u8]) -> (TcpStream, u8) {
            let mut client = connect_to(port).await;
            client.write_all(&[5, 1, 0]).await.unwrap();
            let mut method = [0; 2];
            client.read_exact(&mut method).await.unwrap();
            assert_eq!(method, [5, 0]);
            client.write_all(request).await.unwrap();
            let mut reply = [0; 10];
            tokio::time::timeout(WAIT, client.read_exact(&mut reply)).await.unwrap().unwrap();
            (client, reply[1])
        }

        let [high, low] = target.to_be_bytes();
        let (mut client, code) = ask(port, &[5, 1, 0, 1, 127, 0, 0, 1, high, low]).await;
        assert_eq!(code, socks::SUCCEEDED);
        exchange(&mut client, b"by address").await;
        events.until("proxy", |s| s.connections == 1).await;

        let mut by_name = vec![5, 1, 0, 3, 9];
        by_name.extend(b"localhost");
        by_name.extend([high, low]);
        let (mut named, code) = ask(port, &by_name).await;
        assert_eq!(code, socks::SUCCEEDED);
        exchange(&mut named, b"by name").await;
        events.until("proxy", |s| s.connections == 2).await;

        // Nothing answers there: “connection refused”, and the last failure.
        let [high, low] = free_port().to_be_bytes();
        let nothing = u16::from_be_bytes([high, low]);
        let (_, code) = ask(port, &[5, 1, 0, 1, 127, 0, 0, 1, high, low]).await;
        assert_eq!(code, socks::CONNECTION_REFUSED);
        let state = events.until("proxy", |s| s.last_failure.is_some()).await;
        assert_eq!(
            state.last_failure.unwrap().message,
            format!("Last connection refused: nothing answers at 127.0.0.1:{nothing} on the server.")
        );

        // BIND: “command not supported”.
        let (_, code) = ask(port, &[5, 2, 0, 1, 127, 0, 0, 1, high, low]).await;
        assert_eq!(code, socks::COMMAND_NOT_SUPPORTED);

        drop((client, named));
        events.until("proxy", |s| s.connections == 0).await;
        tunnels.close("g").await;
        assert!(port_is_free(port));
    }
}
