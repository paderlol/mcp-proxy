import { type HTMLAttributes, type ReactNode } from "react";

interface CardProps extends HTMLAttributes<HTMLDivElement> {
  children: ReactNode;
  hoverable?: boolean;
}

export function Card({
  children,
  hoverable = false,
  className = "",
  ...props
}: CardProps) {
  return (
    <div
      className={`bg-bg-surface rounded-lg p-4 ${
        hoverable
          ? "transition-all duration-200 hover:bg-bg-elevated hover:shadow-[rgba(0,0,0,0.3)_0px_8px_8px] cursor-pointer"
          : ""
      } ${className}`}
      {...props}
    >
      {children}
    </div>
  );
}
