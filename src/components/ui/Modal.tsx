import { type ReactNode, useEffect } from "react";
import { X } from "lucide-react";

type ModalSize = "sm" | "md" | "lg" | "xl";

interface ModalProps {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
  size?: ModalSize;
}

const sizeClasses: Record<ModalSize, string> = {
  sm: "min-w-[400px] max-w-md",
  md: "min-w-[480px] max-w-lg",
  lg: "min-w-[560px] max-w-2xl",
  xl: "min-w-[640px] max-w-3xl",
};

export function Modal({
  open,
  onClose,
  title,
  children,
  size = "md",
}: ModalProps) {
  useEffect(() => {
    const handleEsc = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    if (open) document.addEventListener("keydown", handleEsc);
    return () => document.removeEventListener("keydown", handleEsc);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 bg-black/60" onClick={onClose} />
      <div
        className={`relative bg-bg-card-alt rounded-lg p-6 ${sizeClasses[size]} max-h-[90vh] overflow-y-auto shadow-[rgba(0,0,0,0.5)_0px_8px_24px] z-10`}
      >
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-bold text-text-primary">{title}</h2>
          <button
            onClick={onClose}
            className="text-text-secondary hover:text-text-primary transition-colors p-1 rounded-full hover:bg-bg-elevated"
          >
            <X size={18} />
          </button>
        </div>
        {children}
      </div>
    </div>
  );
}
