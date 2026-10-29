// Single palette for Ant Design. Keep in sync with the --ui-* variables in src/styles/globals.css.
export const uiPalette = {
  light: {
    canvas: "#f3f4f6",
    surface: "#ffffff",
    surface2: "#f7f8fa",
    line: "#e3e6ea",
    lineStrong: "#cfd4da",
    fg: "#1d2129",
    fg2: "#4e5969",
    fg3: "#86909c",
    primary: "#2f6bd9",
    success: "#1f9254",
    warning: "#b86e00",
    danger: "#d1453b",
  },
  dark: {
    canvas: "#17181a",
    surface: "#1f2023",
    surface2: "#25262a",
    line: "#303236",
    lineStrong: "#3c3f45",
    fg: "#e3e5e8",
    fg2: "#aeb3ba",
    fg3: "#80858d",
    primary: "#5b8cf0",
    success: "#3fb26f",
    warning: "#e0a84a",
    danger: "#f07167",
  },
} as const;

export type UiPalette = (typeof uiPalette)[keyof typeof uiPalette];

export const uiFontFamily = `"MiSans","Segoe UI","Microsoft YaHei UI","PingFang SC","Noto Sans SC",sans-serif`;
