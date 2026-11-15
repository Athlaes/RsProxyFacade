export interface Settings {
  categories: Categorie[];
}

export type Categorie = {
  name: string;
  display: string;
  parameters: Parameter[];
};

export type ParameterName =
  | "startProxyServerOnAppStart"
  | "startPacServerOnAppStart"
  | "startAppOnOsStart"
  | "minimizeToTaskbar"
  | "forceSystemToUseExposedProxyPac";

export interface Parameter {
  type: "bool";
  label: string;
  name: ParameterName;
  value: string | string[] | boolean | { key: string; value: string }[];
}

export const settings: Settings = {
  categories: [
    {
      name: "general",
      display: "Général",
      parameters: [
        {
          type: "bool",
          label: "Start app on OS start",
          name: "startAppOnOsStart",
          value: false,
        },
        {
          type: "bool",
          label: "Minimize to taskbar on minimize",
          name: "minimizeToTaskbar",
          value: false,
        },
        {
          type: "bool",
          label: "Start proxy server when app start",
          name: "startProxyServerOnAppStart",
          value: false,
        },
        {
          type: "bool",
          label: "Start proxy pac server when app start",
          name: "startPacServerOnAppStart",
          value: false,
        },
        {
          type: "bool",
          label: "Force system to use exposed proxy pac when server is started",
          name: "forceSystemToUseExposedProxyPac",
          value: false,
        },
      ],
    },
  ],
};
