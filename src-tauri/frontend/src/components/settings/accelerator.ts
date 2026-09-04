/**
 * 全局快捷键 accelerator 工具：KeyboardEvent → accelerator 字符串（与 Rust 侧
 * tauri-plugin-global-shortcut 格式一致：修饰键 + 主键，`+` 分隔），以及
 * accelerator → 展示文本（mac 符号 / 其他平台全名）。
 *
 * 主键格式：字母/数字用单字符（Z、1），其余用 Code 名（Comma、Space…）。
 * 修饰键统一生成 CmdOrCtrl / Alt / Shift（跨平台由插件映射到 Cmd 或 Ctrl）。
 */

export interface ShortcutLikeEvent {
  code: string;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
}

/** code → accelerator 主键段；未列出的 code 不支持自定义（返回 null 忽略）。 */
const CODE_TO_MAIN: Record<string, string> = {
  Space: "Space",
  Comma: "Comma",
  Period: "Period",
  Slash: "Slash",
  Semicolon: "Semicolon",
  Quote: "Quote",
  BracketLeft: "BracketLeft",
  BracketRight: "BracketRight",
  Backslash: "Backslash",
  Minus: "Minus",
  Equal: "Equal",
  Backquote: "Backquote",
  Tab: "Tab",
  Enter: "Enter",
};

function mainKeyFromCode(code: string): string | null {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3); // KeyZ → Z
  if (/^Digit\d$/.test(code)) return code.slice(5); // Digit1 → 1
  return CODE_TO_MAIN[code] ?? null;
}

/** 从键盘事件构造 accelerator；裸键（无修饰键）或不支持的主键返回 null。 */
export function acceleratorFromEvent(e: ShortcutLikeEvent): string | null {
  const main = mainKeyFromCode(e.code);
  if (!main) return null;
  const mods: string[] = [];
  if (e.metaKey || e.ctrlKey) mods.push("CmdOrCtrl");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  if (mods.length === 0) return null;
  return [...mods, main].join("+");
}

const MOD_DISPLAY_WIN: Record<string, string> = {
  CmdOrCtrl: "Ctrl",
  Alt: "Alt",
  Shift: "Shift",
};

const MOD_SYMBOL_MAC: Record<string, string> = {
  CmdOrCtrl: "⌘",
  Alt: "⌥",
  Shift: "⇧",
};

const MAIN_DISPLAY: Record<string, string> = {
  Comma: ",",
  Period: ".",
  Slash: "/",
  Semicolon: ";",
  Quote: "'",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Minus: "-",
  Equal: "=",
  Backquote: "`",
  Space: "Space",
  Tab: "Tab",
  Enter: "Enter",
};

/** accelerator → 展示文本。mac 用符号拼接种类（⌘⇧V），其余平台用 + 连接全名。 */
export function formatAccelerator(accelerator: string, isMac: boolean): string {
  const parts = accelerator.split("+");
  const main = parts[parts.length - 1];
  const mods = parts.slice(0, -1);
  const mainDisplay = MAIN_DISPLAY[main] ?? main;
  if (!isMac) {
    return [...mods.map((m) => MOD_DISPLAY_WIN[m] ?? m), mainDisplay].join("+");
  }
  const symbols = mods.map((m) => MOD_SYMBOL_MAC[m] ?? m).join("");
  return `${symbols}${mainDisplay}`;
}
