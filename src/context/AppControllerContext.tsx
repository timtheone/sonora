import { createContext, useContext, type PropsWithChildren } from "react";
import { useAppController, type AppController } from "../hooks/useAppController";

const AppControllerContext = createContext<AppController | null>(null);

export function AppControllerProvider({ children }: PropsWithChildren) {
  const controller = useAppController();
  return <AppControllerContext.Provider value={controller}>{children}</AppControllerContext.Provider>;
}

export function useAppControllerContext() {
  const value = useContext(AppControllerContext);
  if (!value) {
    throw new Error("useAppControllerContext must be used within AppControllerProvider");
  }
  return value;
}
