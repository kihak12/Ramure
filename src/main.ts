import { mount } from 'svelte';
// The fonts of the guidelines, bundled with the application: nothing is loaded from the network.
import '@fontsource-variable/dm-sans';
import '@fontsource/dm-mono/400.css';
import '@fontsource/dm-mono/500.css';
import './app.css';
import App from './App.svelte';
import { theme } from './lib/settings/theme.svelte';

// The theme chosen in the settings is applied before the first paint.
theme.apply();

const app = mount(App, { target: document.getElementById('app')! });

export default app;
