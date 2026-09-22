import type { InputHTMLAttributes } from "react";
import { cn } from "./cn";

type InputVariant = "default" | "inline";

const VARIANTS: Record<InputVariant, string> = {
  default:
    "h-9 rounded-xl border border-border bg-background px-3 py-1.5 transition-[border-color,box-shadow] duration-150 ease-standard focus:border-primary focus:ring-2 focus:ring-primary/15",
  inline:
    "h-8 rounded-none border-x-0 border-t-0 border-b border-transparent bg-transparent px-0 py-0 transition-[border-color] duration-150 ease-standard focus:border-primary",
};

export type InputProps = InputHTMLAttributes<HTMLInputElement> & {
  variant?: InputVariant;
};

export function Input({ variant = "default", className, ...props }: InputProps) {
  return (
    <input
      className={cn(
        "w-full font-sans text-sm font-normal text-text outline-none placeholder:text-muted disabled:cursor-default disabled:opacity-45",
        VARIANTS[variant],
        className,
      )}
      {...props}
    />
  );
}
