import type { ButtonHTMLAttributes, HTMLAttributes } from "react";
import { motion, useReducedMotion } from "motion/react";
import { cn } from "./cn";

const BASE = [
  "relative h-6 w-11 flex-none rounded-full border border-border bg-surface",
  "transition-[background,border-color] duration-150 ease-standard",
  "hover:border-border-strong",
  "disabled:cursor-default disabled:opacity-45 focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-text focus-visible:outline-offset-2",
].join(" ");

function classes(checked: boolean, className?: string) {
  return cn(
    BASE,
    checked && "border-primary bg-primary",
    className,
  );
}

export function Switch({ checked = false, className, children, ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { checked?: boolean }) {
  const reduced = useReducedMotion();
  return (
    <button role="switch" aria-checked={checked} className={classes(checked, className)} {...props}>
      <motion.span
        className="pointer-events-none block rounded-full bg-background shadow-md"
        initial={false}
        animate={{
          x: checked ? 22 : 2,
          width: 18,
          height: 18,
          y: 2,
        }}
        transition={
          reduced
            ? { duration: 0 }
            : { type: "spring", stiffness: 500, damping: 30 }
        }
      />
      {children}
    </button>
  );
}

export function SwitchIndicator({ checked = false, className, ...props }: HTMLAttributes<HTMLSpanElement> & { checked?: boolean }) {
  return (
    <span className={classes(checked, className)} {...props}>
      <span className="pointer-events-none absolute start-[3px] top-[3px] h-3.5 w-3.5 rounded-full bg-background shadow-sm transition-[translate] duration-150 ease-standard data-[checked=true]:translate-x-4" data-checked={checked}>
        <span />
      </span>
    </span>
  );
}
