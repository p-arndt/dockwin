// Svelte 5 bootstrap — this file ONLY mounts the app. No view/business logic here.
import { mount } from "svelte";
import "./app.css";
import App from "./App.svelte";

const target = document.getElementById("app");
if (!target) throw new Error("Mount target #app not found");

const app = mount(App, { target });

export default app;
