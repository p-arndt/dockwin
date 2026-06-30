import { redirect } from "@sveltejs/kit";

// The root has no screen of its own — Containers is the home view.
export function load() {
  redirect(307, "/containers");
}
