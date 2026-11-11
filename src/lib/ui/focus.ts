// Un focus donné par le script, à la suite d'une action au clavier ou d'un événement que
// l'utilisateur n'a pas déclenché à la souris, n'a pas toujours l'anneau du navigateur : WebKit ne
// le montre que si l'élément quitté le montrait. On le pose, le temps du focus (classe
// `.focus-ring`, app.css) : un focus se voit toujours (charte, section 11).
export function focusWithRing(element: HTMLElement | null | undefined) {
  if (!element) return;
  element.focus();
  element.classList.add('focus-ring');
  element.addEventListener('blur', () => element.classList.remove('focus-ring'), { once: true });
}
