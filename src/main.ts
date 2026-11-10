import { mount } from 'svelte';
// Polices de la charte, embarquées dans l'application : rien n'est chargé du réseau.
import '@fontsource-variable/dm-sans';
import '@fontsource/dm-mono/400.css';
import '@fontsource/dm-mono/500.css';
import './app.css';
import App from './App.svelte';

const app = mount(App, { target: document.getElementById('app')! });

export default app;
