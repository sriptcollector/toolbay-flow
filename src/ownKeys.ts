import { api } from "./api";

// Windows doesn't send keystrokes typed into Flow's own windows to Flow's global
// keyboard hook, so the dictation hotkey did nothing while Flow was focused.
// Forward them to the same hotkey engine instead (Windows only; codes are virtual keys).
export function forwardOwnKeys() {
  if (navigator.userAgent.includes("Mac")) return;
  const vkOf = (e: KeyboardEvent): number => {
    switch (e.key) {
      case "Control":
        return e.location === 2 ? 0xa3 : 0x11;
      case "Alt":
      case "AltGraph":
        return e.location === 2 ? 0xa5 : 0x12;
      case "Shift":
        return 0x10;
      case "Meta":
      case "OS":
        return 0x5b;
      default:
        return e.keyCode;
    }
  };
  const send = (e: KeyboardEvent, down: boolean) => {
    const vk = vkOf(e);
    if (vk) api.ownKey(vk, down).catch(() => {});
  };
  window.addEventListener("keydown", (e) => send(e, true), true);
  window.addEventListener("keyup", (e) => send(e, false), true);
  // Releasing keys while the window loses focus would leave them "held".
  window.addEventListener("blur", () => [0x11, 0xa3, 0x12, 0xa5, 0x10, 0x5b].forEach((vk) => api.ownKey(vk, false).catch(() => {})));
}
