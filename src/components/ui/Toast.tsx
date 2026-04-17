import { useEffect } from "react";
import { CheckCircle, XCircle, AlertTriangle, Info } from "lucide-react";

type ToastType = "success" | "error" | "warning" | "info";

interface ToastProps {
  message: string;
  type?: ToastType;
  onDismiss: () => void;
  duration?: number;
}

const icons: Record<ToastType, typeof CheckCircle> = {
  success: CheckCircle,
  error: XCircle,
  warning: AlertTriangle,
  info: Info,
};

const colors: Record<ToastType, string> = {
  success: "text-brand",
  error: "text-negative",
  warning: "text-warning",
  info: "text-info",
};

export function Toast({
  message,
  type = "info",
  onDismiss,
  duration = 3000,
}: ToastProps) {
  useEffect(() => {
    const timer = setTimeout(onDismiss, duration);
    return () => clearTimeout(timer);
  }, [onDismiss, duration]);

  const Icon = icons[type];

  return (
    <div className="fixed bottom-6 right-6 z-50 flex items-center gap-3 bg-bg-card-alt rounded-lg px-4 py-3 shadow-[rgba(0,0,0,0.5)_0px_8px_24px] animate-[slideUp_0.2s_ease-out]">
      <Icon size={18} className={colors[type]} />
      <span className="text-sm text-text-primary">{message}</span>
    </div>
  );
}
