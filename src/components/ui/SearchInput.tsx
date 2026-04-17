import { type InputHTMLAttributes } from "react";
import { Search } from "lucide-react";

export function SearchInput({
  className = "",
  ...props
}: InputHTMLAttributes<HTMLInputElement>) {
  return (
    <div className="relative">
      <Search
        size={16}
        className="absolute left-4 top-1/2 -translate-y-1/2 text-text-secondary"
      />
      <input
        type="text"
        className={`w-full bg-bg-elevated text-text-primary rounded-[500px] pl-11 pr-4 py-3 text-sm outline-none border border-transparent focus:border-border-default shadow-[rgb(18,18,18)_0px_1px_0px,rgb(124,124,124)_0px_0px_0px_1px_inset] placeholder:text-text-secondary/50 ${className}`}
        {...props}
      />
    </div>
  );
}
