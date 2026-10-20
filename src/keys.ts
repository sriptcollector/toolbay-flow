// Display names for shortcut key codes (Windows virtual keys / macOS key codes).
const isMac = navigator.userAgent.includes("Mac");

const WIN: Record<number, string> = {
  0x08: "Backspace", 0x09: "Tab", 0x0d: "Enter", 0x10: "Shift", 0x11: "Ctrl", 0x12: "Alt", 0x13: "Pause",
  0x14: "Caps Lock", 0x1b: "Esc", 0x20: "Space", 0x21: "Page Up", 0x22: "Page Down", 0x23: "End", 0x24: "Home",
  0x25: "←", 0x26: "↑", 0x27: "→", 0x28: "↓", 0x2d: "Insert", 0x2e: "Delete", 0x5b: "Win", 0x5d: "Menu", 0xa3: "Right Ctrl", 0xa5: "Right Alt",
  0xba: ";", 0xbb: "=", 0xbc: ",", 0xbd: "-", 0xbe: ".", 0xbf: "/", 0xc0: "`", 0xdb: "[", 0xdc: "\\", 0xdd: "]", 0xde: "'",
};
const MAC: Record<number, string> = {
  63: "Fn", 59: "Control", 58: "Option", 55: "⌘", 56: "Shift", 62: "Right Control", 61: "Right Option", 54: "Right ⌘", 49: "Space", 53: "Esc", 36: "Return", 48: "Tab", 51: "Delete",
  0: "A", 1: "S", 2: "D", 3: "F", 4: "H", 5: "G", 6: "Z", 7: "X", 8: "C", 9: "V", 11: "B", 12: "Q", 13: "W", 14: "E", 15: "R",
  16: "Y", 17: "T", 18: "1", 19: "2", 20: "3", 21: "4", 22: "6", 23: "5", 25: "9", 26: "7", 28: "8", 29: "0", 31: "O", 32: "U",
  34: "I", 35: "P", 37: "L", 38: "J", 40: "K", 45: "N", 46: "M",
  122: "F1", 120: "F2", 99: "F3", 118: "F4", 96: "F5", 97: "F6", 98: "F7", 100: "F8", 101: "F9", 109: "F10", 103: "F11", 111: "F12",
};
const ORDER_WIN = [0x11, 0xa3, 0x5b, 0x12, 0xa5, 0x10];
const ORDER_MAC = [63, 59, 62, 58, 61, 55, 54, 56];

export function keyName(code: number): string {
  if (isMac) return MAC[code] ?? `Key ${code}`;
  if (WIN[code]) return WIN[code];
  if (code >= 0x30 && code <= 0x5a) return String.fromCharCode(code);
  if (code >= 0x70 && code <= 0x87) return `F${code - 0x6f}`;
  if (code >= 0x60 && code <= 0x69) return `Num ${code - 0x60}`;
  return `Key ${code}`;
}

export function sortKeys(keys: number[]): number[] {
  const order = isMac ? ORDER_MAC : ORDER_WIN;
  return [...keys].sort((a, b) => {
    const ia = order.indexOf(a), ib = order.indexOf(b);
    return (ia < 0 ? 99 : ia) - (ib < 0 ? 99 : ib) || a - b;
  });
}

export function comboLabel(keys: number[]): string {
  return sortKeys(keys).map(keyName).join(" + ");
}

export { isMac };
