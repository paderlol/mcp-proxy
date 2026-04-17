import { useState, useEffect, useCallback } from "react";
import { MainContent } from "../components/layout/MainContent";
import { Card } from "../components/ui/Card";
import { PillButton } from "../components/ui/PillButton";
import { Modal } from "../components/ui/Modal";
import {
  Copy,
  Check,
  Info,
  FileText,
  AlertTriangle,
  CheckCircle2,
} from "lucide-react";
import {
  generateConfig,
  getClientConfigInfo,
  writeClientConfig,
} from "../lib/tauri";
import type { ClientConfigInfo, WriteConfigResult } from "../lib/types";

type ClientType = "claude" | "codex" | "cursor" | "vscode" | "windsurf";

interface ClientUi {
  label: string;
  file: string;
  location: string;
}

const clients: Record<ClientType, ClientUi> = {
  claude: {
    label: "Claude Desktop",
    file: "claude_desktop_config.json",
    location: "~/Library/Application Support/Claude/",
  },
  codex: {
    label: "Codex",
    file: "config.toml",
    location: "~/.codex/",
  },
  cursor: {
    label: "Cursor",
    file: "mcp.json",
    location: "~/.cursor/",
  },
  vscode: {
    label: "VS Code",
    file: "mcp.json",
    location: ".vscode/",
  },
  windsurf: {
    label: "Windsurf",
    file: "mcp_config.json",
    location: "~/.codeium/windsurf/",
  },
};

export function ConfigGenerator() {
  const [selectedClient, setSelectedClient] = useState<ClientType>("claude");
  const [config, setConfig] = useState("");
  const [copied, setCopied] = useState(false);
  const [loading, setLoading] = useState(false);

  const [clientInfo, setClientInfo] = useState<ClientConfigInfo | null>(null);
  const [showConfirm, setShowConfirm] = useState(false);
  const [writing, setWriting] = useState(false);
  const [writeResult, setWriteResult] = useState<WriteConfigResult | null>(
    null,
  );
  const [writeError, setWriteError] = useState<string | null>(null);

  const fetchAll = useCallback(async (client: ClientType) => {
    setLoading(true);
    // Clear any previous write banners when switching clients
    setWriteResult(null);
    setWriteError(null);
    try {
      const [cfg, info] = await Promise.all([
        generateConfig(client).catch(() => "// Error generating config — add servers first"),
        getClientConfigInfo(client).catch(() => null),
      ]);
      setConfig(cfg);
      setClientInfo(info);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchAll(selectedClient);
  }, [selectedClient, fetchAll]);

  const client = clients[selectedClient];

  const copyToClipboard = () => {
    navigator.clipboard.writeText(config);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleConfirmWrite = async () => {
    setShowConfirm(false);
    setWriting(true);
    setWriteError(null);
    setWriteResult(null);
    try {
      const result = await writeClientConfig(selectedClient);
      setWriteResult(result);
      // Re-fetch info so `exists: true` is reflected
      try {
        const info = await getClientConfigInfo(selectedClient);
        setClientInfo(info);
      } catch {
        /* ignore refresh error */
      }
    } catch (err) {
      setWriteError(String(err));
    } finally {
      setWriting(false);
    }
  };

  const canWrite = clientInfo?.supported ?? false;

  return (
    <MainContent
      title="Config"
      description="Generate MCP config for your AI client"
    >
      <Card className="mb-6">
        <div className="flex items-start gap-3">
          <div className="p-2 rounded-lg bg-bg-elevated text-info">
            <Info size={18} />
          </div>
          <div>
            <p className="text-sm font-bold text-text-primary mb-1">
              How It Works
            </p>
            <p className="text-xs text-text-secondary leading-relaxed">
              MCP Proxy generates config where each server's command points to{" "}
              <code className="text-text-bright">
                mcp-proxy run &lt;server-id&gt;
              </code>
              . Secrets are resolved at runtime and never appear in config
              files. Your existing (non-mcp-proxy) entries are preserved on
              write.
            </p>
          </div>
        </div>
      </Card>

      <div className="mb-6">
        <label className="text-sm text-text-secondary font-bold mb-2 block">
          Target Client
        </label>
        <div className="flex flex-wrap gap-2">
          {(Object.entries(clients) as [ClientType, ClientUi][]).map(
            ([key, info]) => (
              <PillButton
                key={key}
                variant={selectedClient === key ? "dark" : "outlined"}
                onClick={() => setSelectedClient(key)}
              >
                {info.label}
              </PillButton>
            ),
          )}
        </div>
      </div>

      <Card>
        <div className="flex items-center justify-between mb-1">
          <div>
            <p className="text-sm font-bold text-text-primary">{client.file}</p>
            <p className="text-xs text-text-secondary mt-0.5">
              {clientInfo?.path ?? client.location}
              {clientInfo?.exists && (
                <span className="ml-2 text-text-secondary/60">· file exists</span>
              )}
            </p>
          </div>
          <div className="flex items-center gap-2">
            <PillButton
              variant="outlined"
              onClick={copyToClipboard}
              className="!py-1 !px-3 !text-xs"
            >
              {copied ? (
                <Check size={12} className="mr-1 text-brand" />
              ) : (
                <Copy size={12} className="mr-1" />
              )}
              {copied ? "Copied" : "Copy"}
            </PillButton>
            {canWrite && (
              <PillButton
                variant="brand"
                onClick={() => setShowConfirm(true)}
                disabled={writing}
                className="!py-1 !px-3 !text-xs"
              >
                <FileText size={12} className="mr-1" />
                {writing ? "Writing…" : "Write to File"}
              </PillButton>
            )}
          </div>
        </div>
        <pre className="bg-bg-base rounded-md p-3 mt-3 text-xs text-text-bright font-mono overflow-x-auto whitespace-pre min-h-[80px]">
          {loading ? "Loading..." : config || "// No servers configured yet"}
        </pre>

        {/* Unsupported banner (e.g., VS Code) */}
        {clientInfo && !clientInfo.supported && clientInfo.unsupported_reason && (
          <div className="mt-3 rounded-md bg-bg-elevated border border-warning/30 p-3 flex items-start gap-2">
            <AlertTriangle size={14} className="text-warning flex-shrink-0 mt-0.5" />
            <p className="text-xs text-text-secondary leading-relaxed">
              {clientInfo.unsupported_reason}
            </p>
          </div>
        )}

        {/* Write success banner */}
        {writeResult && (
          <div className="mt-3 rounded-md bg-bg-elevated border border-brand/30 p-3 flex items-start gap-2">
            <CheckCircle2 size={14} className="text-brand flex-shrink-0 mt-0.5" />
            <div className="flex-1 text-xs text-text-secondary leading-relaxed">
              <p className="text-text-primary font-bold mb-1">
                Wrote {writeResult.managed_count} server
                {writeResult.managed_count === 1 ? "" : "s"}
              </p>
              <p>
                Path: <code className="text-text-bright">{writeResult.path}</code>
              </p>
              {writeResult.preserved_count > 0 && (
                <p>
                  Preserved {writeResult.preserved_count} of your existing
                  non-mcp-proxy entries.
                </p>
              )}
              {writeResult.backup_path && (
                <p>
                  Backup:{" "}
                  <code className="text-text-bright">
                    {writeResult.backup_path}
                  </code>
                </p>
              )}
              <p className="mt-1 text-text-secondary/70">
                Restart the client to pick up the new config.
              </p>
            </div>
          </div>
        )}

        {/* Write error banner */}
        {writeError && (
          <div className="mt-3 rounded-md bg-bg-elevated border border-negative/30 p-3 flex items-start gap-2">
            <AlertTriangle size={14} className="text-negative flex-shrink-0 mt-0.5" />
            <div className="flex-1 text-xs text-text-secondary leading-relaxed">
              <p className="text-text-primary font-bold mb-1">Write failed</p>
              <p className="font-mono text-text-bright">{writeError}</p>
              <p className="mt-1 text-text-secondary/70">
                The config file was not modified. You can still use Copy to
                paste manually.
              </p>
            </div>
          </div>
        )}

        <p className="text-xs text-text-secondary/60 mt-3">
          No secrets in this file — resolved at runtime by{" "}
          <code className="text-text-bright">mcp-proxy run</code>
        </p>
      </Card>

      <Modal
        open={showConfirm}
        onClose={() => setShowConfirm(false)}
        title={`Write to ${client.label}?`}
      >
        <div className="flex flex-col gap-3 text-sm">
          <p className="text-text-secondary">
            About to write MCP Proxy's server entries to:
          </p>
          <p className="bg-bg-elevated rounded-md p-3 font-mono text-xs text-text-bright break-all">
            {clientInfo?.path ?? "(resolving path…)"}
          </p>
          <ul className="text-xs text-text-secondary space-y-1 list-disc list-inside">
            <li>Existing non-mcp-proxy server entries will be preserved.</li>
            <li>
              Stale mcp-proxy entries (servers you removed here) will be
              dropped.
            </li>
            {clientInfo?.exists && (
              <li>
                A timestamped backup (<code>.backup-YYYYMMDDTHHMMSS</code>)
                will be created first.
              </li>
            )}
            <li>You'll need to restart {client.label} to load changes.</li>
          </ul>
          <div className="flex justify-end gap-2 mt-2">
            <PillButton variant="outlined" onClick={() => setShowConfirm(false)}>
              Cancel
            </PillButton>
            <PillButton variant="brand" onClick={handleConfirmWrite}>
              Write
            </PillButton>
          </div>
        </div>
      </Modal>
    </MainContent>
  );
}
