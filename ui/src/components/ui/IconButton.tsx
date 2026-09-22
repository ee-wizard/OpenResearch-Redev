import { forwardRef } from "react";
import { motion, useReducedMotion, type HTMLMotionProps } from "motion/react";
import { cn } from "./cn";

const BASE = [
  "icon-btn relative inline-flex shrink-0 items-center justify-center",
  "transition-[background,color,transform] duration-150 ease-standard motion-safe:duration-150",
  "focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-text focus-visible:outline-offset-2",
  "disabled:cursor-default disabled:opacity-45",
].join(" ");

type IconButtonSize = "default" | "small";
type IconButtonVariant = "default" | "primary" | "stop";

const VARIANTS: Record<IconButtonVariant, string> = {
  default: "text-subtext [&:hover:not(:disabled)]:bg-surface [&:hover:not(:disabled)]:text-text [&:active:not(:disabled)]:bg-highlight [&.active]:bg-surface [&.active]:text-primary",
  primary: "bg-primary text-background [&:hover:not(:disabled)]:bg-primary-hover [&:active:not(:disabled)]:bg-primary-active",
  stop: "bg-surface text-text [&:hover:not(:disabled)]:bg-stop-hover [&:active:not(:disabled)]:bg-highlight",
};

const SIZES: Record<IconButtonSize, string> = {
  default: "h-8 w-8 rounded-lg",
  small: "h-7 w-7 rounded-md",
};

function classes(variant: IconButtonVariant, size: IconButtonSize, active: boolean, className?: string) {
  return cn(BASE, VARIANTS[variant], SIZES[size], active && "active", className);
}

export type IconButtonProps = HTMLMotionProps<"button"> & {
  active?: boolean;
  size?: IconButtonSize;
  variant?: IconButtonVariant;
};

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(function IconButton(
  { active = false, size = "default", variant = "default", className, disabled, ...props },
  ref,
) {
  const reduced = useReducedMotion();
  return (
    <motion.button
      ref={ref}
      className={classes(variant, size, active, className)}
      disabled={disabled}
      whileHover={disabled || reduced ? undefined : { scale: 1.08 }}
      whileTap={disabled || reduced ? undefined : { scale: 0.92 }}
      transition={{ type: "spring", stiffness: 420, damping: 18 }}
      {...props}
    />
  );
});

export type IconButtonLinkProps = HTMLMotionProps<"a"> & {
  active?: boolean;
  size?: IconButtonSize;
  variant?: IconButtonVariant;
};

export function IconButtonLink({ active = false, size = "default", variant = "default", className, ...props }: IconButtonLinkProps) {
  const reduced = useReducedMotion();
  return (
    <motion.a
      className={classes(variant, size, active, className)}
      whileHover={reduced ? undefined : { scale: 1.08 }}
      whileTap={reduced ? undefined : { scale: 0.92 }}
      transition={{ type: "spring", stiffness: 420, damping: 18 }}
      {...props}
    />
  );
}
