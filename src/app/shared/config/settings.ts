export interface Settings {
  categories: Categorie[];
}

export type Categorie = {
  name: string;
  display: string;
  parameters: Parameter[];
};

export interface Parameter {
  type: "bool";
  label: string;
  name: string;
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
          label: "Start proxy server when app start",
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
          label: "Start proxy server when app start",
          name: "startPacServerOnAppStart",
          value: false,
        },
        {
          type: "bool",
          label: "startAppOnOsStart",
          name: "startAppOnOsStart",
          value: false,
        },
        {
          type: "bool",
          label: "Start proxy server when app start",
          name: "minimizeToTaskbar",
          value: false,
        },
      ],
    },
  ],
};
