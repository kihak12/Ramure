//! The SOCKS5 server of a SOCKS tunnel (RFC 1928): no authentication, the CONNECT command only,
//! to an IPv4 address, an IPv6 address or a domain name. Each request becomes a `direct-tcpip`
//! channel through the server (`tunnels`).

use std::io;
use std::net::{Ipv4Addr, Ipv6Addr};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Reply codes.
pub const SUCCEEDED: u8 = 0x00;
pub const GENERAL_FAILURE: u8 = 0x01;
pub const CONNECTION_REFUSED: u8 = 0x05;
pub const COMMAND_NOT_SUPPORTED: u8 = 0x07;
pub const ADDRESS_TYPE_NOT_SUPPORTED: u8 = 0x08;

const VERSION: u8 = 5;
const NO_AUTHENTICATION: u8 = 0x00;
const NO_ACCEPTABLE_METHOD: u8 = 0xff;
const CONNECT: u8 = 0x01;
const IPV4: u8 = 0x01;
const DOMAIN: u8 = 0x03;
const IPV6: u8 = 0x04;

/// Where a client asks to go, as the server will resolve it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub host: String,
    pub port: u16,
}

/// Why a client is not carried any further.
#[derive(Debug)]
pub enum Refusal {
    /// The client ended, or sent less than a request: there is no one to answer.
    Ended,
    /// Not a SOCKS5 client.
    NotSocks5,
    /// The client offers no method without authentication.
    NoMethod,
    /// A request the proxy does not carry, and the reply code that says so.
    Reply(u8),
}

impl From<io::Error> for Refusal {
    fn from(_: io::Error) -> Self {
        Refusal::Ended
    }
}

/// The answer to a request: the code, then an address the client does not use.
pub fn reply(code: u8) -> [u8; 10] {
    [VERSION, code, 0, IPV4, 0, 0, 0, 0, 0, 0]
}

/// Reads the greeting: the version and the methods the client offers.
pub async fn read_greeting<R: AsyncRead + Unpin>(r: &mut R) -> Result<(), Refusal> {
    if r.read_u8().await? != VERSION {
        return Err(Refusal::NotSocks5);
    }
    let count = r.read_u8().await?;
    let mut methods = vec![0; usize::from(count)];
    r.read_exact(&mut methods).await?;
    if methods.contains(&NO_AUTHENTICATION) { Ok(()) } else { Err(Refusal::NoMethod) }
}

/// Reads a request: the command, then the address and the port, big-endian.
pub async fn read_request<R: AsyncRead + Unpin>(r: &mut R) -> Result<Target, Refusal> {
    let mut head = [0; 4];
    r.read_exact(&mut head).await?;
    let [version, command, _reserved, address_type] = head;
    if version != VERSION {
        return Err(Refusal::NotSocks5);
    }
    if command != CONNECT {
        return Err(Refusal::Reply(COMMAND_NOT_SUPPORTED));
    }
    let host = match address_type {
        IPV4 => {
            let mut octets = [0; 4];
            r.read_exact(&mut octets).await?;
            Ipv4Addr::from(octets).to_string()
        }
        IPV6 => {
            let mut octets = [0; 16];
            r.read_exact(&mut octets).await?;
            Ipv6Addr::from(octets).to_string()
        }
        DOMAIN => {
            let length = r.read_u8().await?;
            let mut name = vec![0; usize::from(length)];
            r.read_exact(&mut name).await?;
            // An empty name, or one that is not text, names no host.
            match String::from_utf8(name) {
                Ok(name) if !name.is_empty() => name,
                _ => return Err(Refusal::Reply(ADDRESS_TYPE_NOT_SUPPORTED)),
            }
        }
        _ => return Err(Refusal::Reply(ADDRESS_TYPE_NOT_SUPPORTED)),
    };
    let port = r.read_u16().await?;
    Ok(Target { host, port })
}

/// Answers the greeting and reads the request. A request the proxy does not carry is answered
/// here; the caller answers the others once the server has opened the channel, or refused it.
pub async fn handshake<S: AsyncRead + AsyncWrite + Unpin>(s: &mut S) -> Result<Target, Refusal> {
    let result = async {
        read_greeting(s).await?;
        s.write_all(&[VERSION, NO_AUTHENTICATION]).await?;
        read_request(s).await
    }
    .await;
    match &result {
        Err(Refusal::NoMethod) => {
            let _ = s.write_all(&[VERSION, NO_ACCEPTABLE_METHOD]).await;
        }
        Err(Refusal::Reply(code)) => {
            let _ = s.write_all(&reply(*code)).await;
        }
        _ => {}
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn request(bytes: &[u8]) -> Result<Target, Refusal> {
        read_request(&mut &bytes[..]).await
    }

    fn target(host: &str, port: u16) -> Target {
        Target { host: host.into(), port }
    }

    #[tokio::test]
    async fn a_request_names_an_ipv4_an_ipv6_or_a_domain_target() {
        let ipv4 = [5, 1, 0, 1, 127, 0, 0, 1, 0x15, 0x38];
        assert_eq!(request(&ipv4).await.unwrap(), target("127.0.0.1", 5432));

        let mut ipv6 = vec![5, 1, 0, 4];
        ipv6.extend(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1).octets());
        ipv6.extend([0, 80]);
        assert_eq!(request(&ipv6).await.unwrap(), target("2001:db8::1", 80));

        let mut domain = vec![5, 1, 0, 3, 11];
        domain.extend(b"example.com");
        domain.extend(443u16.to_be_bytes());
        assert_eq!(request(&domain).await.unwrap(), target("example.com", 443));
    }

    #[tokio::test]
    async fn only_connect_is_carried() {
        // BIND and UDP ASSOCIATE.
        for command in [2, 3] {
            let bind = [5, command, 0, 1, 127, 0, 0, 1, 0, 80];
            assert!(matches!(request(&bind).await, Err(Refusal::Reply(COMMAND_NOT_SUPPORTED))));
        }
        let unknown_address = [5, 1, 0, 9, 0, 0];
        assert!(matches!(request(&unknown_address).await, Err(Refusal::Reply(ADDRESS_TYPE_NOT_SUPPORTED))));
        let empty_name = [5, 1, 0, 3, 0, 0, 80];
        assert!(matches!(request(&empty_name).await, Err(Refusal::Reply(ADDRESS_TYPE_NOT_SUPPORTED))));
        let socks4 = [4, 1, 0, 80, 127, 0, 0, 1];
        assert!(matches!(request(&socks4).await, Err(Refusal::NotSocks5)));
    }

    #[tokio::test]
    async fn a_short_read_is_an_end_of_stream() {
        for short in [&[5, 1, 0][..], &[5, 1, 0, 1, 127, 0], &[5, 1, 0, 1, 127, 0, 0, 1, 0], &[5, 1, 0, 3, 11, b'e']] {
            assert!(matches!(request(short).await, Err(Refusal::Ended)), "{short:?}");
        }
        assert!(matches!(read_greeting(&mut &[5, 2, 0][..]).await, Err(Refusal::Ended)));
        assert!(matches!(read_greeting(&mut &[][..]).await, Err(Refusal::Ended)));
    }

    #[tokio::test]
    async fn the_handshake_answers_the_greeting_and_the_refusals() {
        // No authentication offered among others: accepted.
        let (mut client, mut proxy) = tokio::io::duplex(64);
        client.write_all(&[5, 2, 2, 0, 5, 1, 0, 1, 10, 0, 0, 7, 0, 22]).await.unwrap();
        assert_eq!(handshake(&mut proxy).await.unwrap(), target("10.0.0.7", 22));
        let mut answer = [0; 2];
        client.read_exact(&mut answer).await.unwrap();
        assert_eq!(answer, [5, 0]);

        // A command other than CONNECT: answered with its reply code.
        let (mut client, mut proxy) = tokio::io::duplex(64);
        client.write_all(&[5, 1, 0, 5, 2, 0, 1, 10, 0, 0, 7, 0, 22]).await.unwrap();
        assert!(matches!(handshake(&mut proxy).await, Err(Refusal::Reply(COMMAND_NOT_SUPPORTED))));
        let mut answer = [0; 12];
        client.read_exact(&mut answer).await.unwrap();
        assert_eq!(answer, [5, 0, 5, 7, 0, 1, 0, 0, 0, 0, 0, 0]);

        // Authentication required by the client: no acceptable method.
        let (mut client, mut proxy) = tokio::io::duplex(64);
        client.write_all(&[5, 1, 2]).await.unwrap();
        assert!(matches!(handshake(&mut proxy).await, Err(Refusal::NoMethod)));
        let mut answer = [0; 2];
        client.read_exact(&mut answer).await.unwrap();
        assert_eq!(answer, [5, 0xff]);
    }
}
