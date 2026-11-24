import { mount } from 'svelte';
// The fonts of the guidelines, bundled with the application: nothing is loaded from the network.
import '@fontsource-variable/dm-sans';
import '@fontsource/dm-mono/400.css';
import '@fontsource/dm-mono/500.css';
import './app.css';
import App from './App.svelte';
import ConflictWindow from './lib/git/ConflictWindow.svelte';
import { askedOf } from './lib/git/conflictWindow';
import { theme } from './lib/settings/theme.svelte';

// The theme chosen in the settings is applied before the first paint.
theme.apply();

// The window of the conflict editor is the same application, asked for one repository (9.26).
const target = document.getElementById('app')!;
const app = askedOf(location.search) ? mount(ConflictWindow, { target }) : mount(App, { target });

export default app;
