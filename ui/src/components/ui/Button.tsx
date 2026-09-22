import type { HTMLMotionProps } from "motion/react";
import { motion, useReducedMotion } from "motion/react";
import { cn } from "./cn";

type ButtonVariant = "default" | "primary" | "ghost" | "danger" | "warning";
type ButtonSize = "default" | "small" | "large";

const BASE = [
  "btn inline-flex shrink-0 items-center justify-center gap-1.5 whitespace-nowrap border font-medium",
  "transition-[background,border-color,color,transform] duration-150 ease-standard motion-safe:duration-150",
  "focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-text focus-visible:outline-offset-2",
  "disabled:cursor-default disabled:opacity-45",
].join(" ");

const VARIANTS: Record<ButtonVariant, string> = {
  default: "border-border bg-background text-text [&:hover:not(:disabled)]:bg-surface [&:active:not(:disabled)]:bg-highlight",
  primary: "border-primary bg-primary text-background [&:hover:not(:disabled)]:border-primary-hover [&:hover:not(:disabled)]:bg-primary-hover [&:active:not(:disabled)]:border-primary-active [&:active:not(:disabled)]:bg-primary-active",
  ghost: "border-transparent bg-transparent text-text [&:hover:not(:disabled)]:bg-surface [&:active:not(:disabled)]:bg-highlight [&.active]:bg-surface [&.active]:text-muted",
  danger: "border-border bg-background text-accent-red [&:hover:not(:disabled)]:bg-danger-hover [&:active:not(:disabled)]:bg-danger-active",
  warning: "border-accent-amber bg-background text-accent-amber [&:hover:not(:disabled)]:bg-accent-amber-subtle [&:active:not(:disabled)]:bg-highlight",
};

const SIZES: Record<ButtonSize, string> = {
  default: "h-8 rounded-xl px-3.5 text-sm",
  small: "h-7 rounded-lg px-2.5 text-sm",
  large: "h-14 rounded-2xl px-7 text-xl",
};

function classes(variant: ButtonVariant, size: ButtonSize, active: boolean, className?: string) {
  return cn(BASE, VARIANTS[variant], SIZES[size], active && "active", className);
}

export type ButtonProps = HTMLMotionProps<"button"> & {
  active?: boolean;
  variant?: ButtonVariant;
  size?: ButtonSize;
};

export function Button({ active = false, variant = "default", size = "default", className, disabled, ...props }: ButtonProps) {
  const reduced = useReducedMotion();
  return (
    <motion.button
      className={classes(variant, size, active, className)}
      disabled={disabled}
      whileHover={disabled || reduced ? undefined : { scale: 1.02 }}
      whileTap={disabled || reduced ? undefined : { scale: 0.98 }}
      transition={{ type: "spring", stiffness: 420, damping: 18 }}
      {...props}
    />
  );
}

export type ButtonLinkProps = HTMLMotionProps<"a"> & {
  active?: boolean;
  variant?: ButtonVariant;
  size?: ButtonSize;
};

export function ButtonLink({ active = false, variant = "default", size = "default", className, ...props }: ButtonLinkProps) {
  const reduced = useReducedMotion();
  return (
    <motion.a
      className={classes(variant, size, active, className)}
      whileHover={reduced ? undefined : { scale: 1.02 }}
      whileTap={reduced ? undefined : { scale: 0.98 }}
      transition={{ type: "spring", stiffness: 420, damping: 18 }}
      {...props}
    />
  );
}
