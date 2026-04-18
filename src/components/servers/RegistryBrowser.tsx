import { useState } from "react";
import { Modal } from "../ui/Modal";
import { PillButton } from "../ui/PillButton";
import { Badge } from "../ui/Badge";
import { ExternalLink, KeyRound, Download } from "lucide-react";
import {
  filterEntries,
  type RegistryEntry,
  type RegistryRegion,
} from "../../data/registry";

interface RegistryBrowserProps {
  open: boolean;
  onClose: () => void;
  onInstall: (entry: RegistryEntry) => void;
}

const regionLabels: Record<RegistryRegion, string> = {
  international: "Global",
  china: "China",
};

export function RegistryBrowser({
  open,
  onClose,
  onInstall,
}: RegistryBrowserProps) {
  const [region, setRegion] = useState<RegistryRegion>("international");
  const [query, setQuery] = useState("");

  const entries = filterEntries(region, query);

  return (
    <Modal open={open} onClose={onClose} title="Browse MCP Registry" size="lg">
      <div className="flex flex-col gap-4">
        {/* Region tabs */}
        <div className="flex gap-2">
          {(Object.keys(regionLabels) as RegistryRegion[]).map((r) => (
            <PillButton
              key={r}
              variant={region === r ? "brand" : "outlined"}
              onClick={() => setRegion(r)}
              className="flex-1"
            >
              {regionLabels[r]}
            </PillButton>
          ))}
        </div>

        {/* Search */}
        <input
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search by name, publisher, tag, or description..."
          className="w-full bg-bg-elevated text-text-primary rounded-[500px] px-4 py-2.5 text-sm outline-none border border-transparent focus:border-border-default shadow-[rgb(18,18,18)_0px_1px_0px,rgb(124,124,124)_0px_0px_0px_1px_inset] placeholder:text-text-secondary/50"
        />

        {/* Entry list */}
        <div className="flex flex-col gap-2 overflow-y-auto -mx-2 px-2">
          {entries.length === 0 ? (
            <div className="text-center py-12 text-sm text-text-secondary">
              No servers match "{query}"
            </div>
          ) : (
            entries.map((entry) => (
              <EntryCard
                key={entry.id}
                entry={entry}
                onInstall={() => {
                  onInstall(entry);
                  onClose();
                }}
              />
            ))
          )}
        </div>

        <p className="text-xs text-text-secondary/60 text-center">
          Can't find what you need? Use "Add Server" to configure manually.
        </p>
      </div>
    </Modal>
  );
}

function EntryCard({
  entry,
  onInstall,
}: {
  entry: RegistryEntry;
  onInstall: () => void;
}) {
  const secretCount = entry.envVars.filter((v) => v.required).length;

  return (
    <div className="bg-bg-elevated rounded-lg p-4 hover:bg-bg-card transition-colors">
      <div className="flex items-start gap-3">
        <div className="text-2xl leading-none pt-1">{entry.icon ?? "🔌"}</div>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-1 flex-wrap">
            <p className="text-base font-bold text-text-primary">{entry.name}</p>
            {secretCount > 0 && (
              <Badge variant="warning">
                <KeyRound size={10} className="mr-1" />
                {secretCount} secret{secretCount > 1 ? "s" : ""}
              </Badge>
            )}
            {entry.tags.slice(0, 2).map((tag) => (
              <Badge key={tag}>{tag}</Badge>
            ))}
          </div>
          <p className="text-xs text-text-secondary/70 mb-2">
            by <span className="text-text-bright">{entry.publisher}</span>
          </p>
          <p className="text-sm text-text-secondary mb-3 leading-relaxed">
            {entry.description}
          </p>
          <div className="flex items-center gap-3">
            <PillButton variant="brand" onClick={onInstall}>
              <Download size={14} className="mr-1.5" />
              Install
            </PillButton>
            <a
              href={entry.sourceUrl}
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-1 text-sm text-text-secondary hover:text-text-primary transition-colors"
            >
              <ExternalLink size={14} />
              Docs
            </a>
          </div>
        </div>
      </div>
    </div>
  );
}
