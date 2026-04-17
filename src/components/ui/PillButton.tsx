import { type ButtonHTMLAttributes, type ReactNode } from "react";

type Variant = "dark" | "light" | "outlined" | "brand" | "circular";

interface PillButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  children: ReactNode;
}

// Shared sizing: all variants have identical box model to prevent layout shift
// when switching between them (e.g., toggle buttons).
// - Same padding (px-4 py-2)
// - All variants have a 1px border (transparent when not visible)
// - Same text-transform (uppercase) and tracking
const baseSize =
  "px-4 py-2 rounded-[9999px] text-sm font-bold uppercase tracking-[1.4px] border border-transparent";

const variantStyles: Record<Variant, string> = {
  dark: `${baseSize} bg-bg-elevated text-text-primary hover:bg-bg-card`,
  light: `${baseSize} bg-bg-light text-bg-surface`,
  outlined: `${baseSize} bg-transparent text-text-primary !border-border-light hover:!border-text-primary`,
  brand: `${baseSize} bg-brand text-bg-base hover:brightness-110`,
  circular:
    "bg-bg-elevated text-text-primary p-3 rounded-full hover:bg-bg-card border border-transparent",
};

export function PillButton({
  variant = "dark",
  className = "",
  children,
  ...props
}: PillButtonProps) {
  return (
    <button
      className={`inline-flex items-center justify-center transition-all duration-200 cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed ${variantStyles[variant]} ${className}`}
      {...props}
    >
      {children}
    </button>
  );
}
