// Why a connection failed, as the system gives it, said in plain words (guidelines, section
// 9.15). The reasons that are not known are shown as they are, like the output of git.
// The patterns are matched against the text of the system and of the Rust core: they stay as
// that text is written, in English for the system, in French for the core ("ne répond pas").

export function explain(reason: string) {
  if (/refused/i.test(reason)) return 'The server refused the connection: its SSH service may not be running.';
  if (/timed out|ne répond pas/i.test(reason)) return 'The server is not responding.';
  if (/unreachable|no route/i.test(reason)) return 'The network cannot reach the server.';
  if (/reset by peer|broken pipe/i.test(reason)) return 'The connection was cut by the network or by the server.';
  if (/lookup|resolve|not known|no such host|nodename/i.test(reason)) return 'This address cannot be found: check its spelling.';
  return reason;
}
