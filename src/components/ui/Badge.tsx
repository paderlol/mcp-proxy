import { type ReactNode } from "react";

type BadgeVariant = "default" | "success" | "error" | "warning";

interface BadgeProps {
  children: ReactNode;
  variant?: BadgeVariant;
}

const variantStyles: Record<BadgeVariant, string> = {
  default: "bg-bg-elevated text-text-secondary",
  success: "bg-brand/20 text-brand",
  error: "bg-negative/20 text-negative",
  warning: "bg-warning/20 text-warning",
};

export function Badge({ children, variant = "default" }: BadgeProps) {
  return (
    <span
      className={`inline-flex items-center px-2 py-0.5 rounded text-[10.5px] font-semibold capitalize ${variantStyles[variant]}`}
    >
      {children}
    </span>
  );
}
