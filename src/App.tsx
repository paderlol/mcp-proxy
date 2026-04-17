import { useEffect } from "react";
import { Routes, Route } from "react-router-dom";
import { Sidebar } from "./components/layout/Sidebar";
import { Dashboard } from "./pages/Dashboard";
import { ServerConfig } from "./pages/ServerConfig";
import { SecretsManager } from "./pages/SecretsManager";
import { ConfigGenerator } from "./pages/ConfigGenerator";
import { Settings } from "./pages/Settings";
import { useVault } from "./hooks/useVault";
import { useIdleLock } from "./hooks/useIdleLock";
import { useVaultIdleTimeout } from "./hooks/useVaultIdleTimeout";

function App() {
  const { refresh: refreshVault } = useVault();
  const [idleTimeoutMs] = useVaultIdleTimeout();

  // Seed vault status once at app launch so components that read it don't
  // see `null` on first render.
  useEffect(() => {
    refreshVault();
  }, [refreshVault]);

  // Arms the idle-lock timer whenever the vault is unlocked. The hook
  // guards itself on backend/unlocked/timeout, so calling it here is safe
  // on all platforms and in all states.
  useIdleLock(idleTimeoutMs);

  return (
    <div className="flex h-screen w-screen overflow-hidden">
      <Sidebar />
      <Routes>
        <Route path="/" element={<Dashboard />} />
        <Route path="/servers" element={<ServerConfig />} />
        <Route path="/secrets" element={<SecretsManager />} />
        <Route path="/config" element={<ConfigGenerator />} />
        <Route path="/settings" element={<Settings />} />
      </Routes>
    </div>
  );
}

export default App;
