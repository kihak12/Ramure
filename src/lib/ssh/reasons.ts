// Les motifs d'échec d'une connexion, donnés par le système en anglais, dits en français
// (charte, section 9.15). Ceux qu'on ne connaît pas sont montrés tels quels, comme la sortie de git.

const NB = ' '; // espace insécable, avant « : »

export function explain(reason: string) {
  if (/refused/i.test(reason)) return `Le serveur refuse la connexion${NB}: son service SSH est peut-être arrêté.`;
  if (/timed out|ne répond pas/i.test(reason)) return 'Le serveur ne répond pas.';
  if (/unreachable|no route/i.test(reason)) return 'Le réseau ne permet pas de joindre le serveur.';
  if (/reset by peer|broken pipe/i.test(reason)) return 'La connexion a été coupée par le réseau ou par le serveur.';
  if (/lookup|resolve|not known|no such host|nodename/i.test(reason)) return `Cette adresse est introuvable${NB}: vérifie son orthographe.`;
  return reason;
}
