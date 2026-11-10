import { mount } from 'svelte';
// Polices de la charte, embarquées dans l'application : rien n'est chargé du réseau.
import '@fontsource-variable/dm-sans';
import '@fontsource/dm-mono/400.css';
import '@fontsource/dm-mono/500.css';
import './app.css';
import App from './App.svelte';
import { theme } from './lib/settings/theme.svelte';

// Le thème choisi dans les réglages est posé avant le premier affichage.
theme.apply();

const app = mount(App, { target: document.getElementById('app')! });

export default app;
