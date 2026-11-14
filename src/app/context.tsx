import { createContext, useContext } from "react";
import type { AppInfo, Settings, ToolchainStatus, TvInfo } from "../types";

export type Page = "install" | "tvs" | "certs" | "history" | "settings";

export interface AppCtx {
  info: AppInfo;
  settings: Settings;
  /** Reloads settings from the backend after something changed them. */
  refresh: () => Promise<void>;
  /** Samsung tools status (desktop SDK engine); null while loading or on Android. */
  tool: ToolchainStatus | null;
  reloadTool: () => void;
  /** TV confirmed connected in this session. */
  tv: TvInfo | null;
  setTv: (tv: TvInfo | null) => void;
  go: (page: Page) => void;
  /** The SDK engine is selected but its tools are not ready yet. */
  toolsMissing: boolean;
}

export const AppContext = createContext<AppCtx | null>(null);

export function useApp(): AppCtx {
  const ctx = useContext(AppContext);
  if (!ctx) throw new Error("AppContext is missing");
  return ctx;
}
