import { type ReactNode } from "react";

interface MainContentProps {
  title: string;
  description?: string;
  actions?: ReactNode;
  children: ReactNode;
}

export function MainContent({
  title,
  description,
  actions,
  children,
}: MainContentProps) {
  return (
    <main className="flex-1 h-full overflow-y-auto bg-bg-base">
      <div className="max-w-5xl mx-auto px-8 py-8">
        <div className="flex items-start justify-between mb-8">
          <div>
            <h1 className="text-2xl font-bold text-text-primary">{title}</h1>
            {description && (
              <p className="mt-1 text-sm text-text-secondary">{description}</p>
            )}
          </div>
          {actions && <div className="flex items-center gap-2">{actions}</div>}
        </div>
        {children}
      </div>
    </main>
  );
}
