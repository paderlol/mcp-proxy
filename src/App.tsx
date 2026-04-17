import { Routes, Route } from "react-router-dom";
import { Sidebar } from "./components/layout/Sidebar";
import { Dashboard } from "./pages/Dashboard";
import { ServerConfig } from "./pages/ServerConfig";
import { SecretsManager } from "./pages/SecretsManager";
import { ConfigGenerator } from "./pages/ConfigGenerator";
import { Settings } from "./pages/Settings";

function App() {
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
