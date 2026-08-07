import '@fontsource/inter/latin.css';
import '@fontsource/inter/vietnamese.css';
import '@fontsource/jetbrains-mono/latin.css';
import '@fontsource/jetbrains-mono/vietnamese.css';

import { mount } from "svelte";
import App from "./App.svelte";
import "./index.css";

const app = mount(App, {
  target: document.getElementById("app") as HTMLElement,
});

export default app;
