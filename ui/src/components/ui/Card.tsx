import { forwardRef } from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { motion, useReducedMotion, type HTMLMotionProps } from "motion/react";
import { cn } from "./cn";

const cardVariants = cva("relative overflow-hidden", {
  variants: {
    variant: {
      default:
        "bg-background border border-border rounded-2xl shadow-card",
      elevated:
        "bg-background border border-border rounded-2xl shadow-elevated",
      interactive:
        "bg-background border border-border rounded-2xl shadow-card transition-[transform,box-shadow] duration-200 ease-standard hover:shadow-elevated motion-safe:hover:-translate-y-0.5 cursor-pointer",
      ghost: "bg-transparent border border-transparent rounded-2xl",
    },
    padding: {
      none: "",
      sm: "p-3",
      default: "p-4",
      lg: "p-6",
    },
  },
  defaultVariants: {
    variant: "default",
    padding: "default",
  },
});

export type CardProps = HTMLMotionProps<"div"> &
  VariantProps<typeof cardVariants> & {
    hover?: "none" | "lift" | "scale";
  };

export const Card = forwardRef<HTMLDivElement, CardProps>(function Card(
  {
    className,
    variant = "default",
    padding = "default",
    hover = "none",
    children,
    ...props
  },
  ref,
) {
  const reduced = useReducedMotion();
  const base = cardVariants({ variant, padding });
  const motionHover =
    reduced || hover === "none"
      ? undefined
      : hover === "lift"
        ? { y: -6 }
        : { scale: 1.02 };

  return (
    <motion.div
      ref={ref}
      className={cn(base, className)}
      whileHover={motionHover}
      whileTap={reduced ? undefined : { scale: 0.98 }}
      transition={{ type: "spring", stiffness: 320, damping: 22 }}
      {...props}
    >
      {children}
    </motion.div>
  );
});

export type CardHeaderProps = React.ComponentProps<"div">;
export function CardHeader({ className, ...props }: CardHeaderProps) {
  return <div className={cn("flex flex-col gap-1.5", className)} {...props} />;
}

export type CardTitleProps = React.ComponentProps<"h3">;
export function CardTitle({ className, ...props }: CardTitleProps) {
  return (
    <h3 className={cn("text-base font-semibold text-text", className)} {...props} />
  );
}

export type CardDescriptionProps = React.ComponentProps<"p">;
export function CardDescription({ className, ...props }: CardDescriptionProps) {
  return (
    <p className={cn("text-sm text-subtext", className)} {...props} />
  );
}

export type CardContentProps = React.ComponentProps<"div">;
export function CardContent({ className, ...props }: CardContentProps) {
  return <div className={cn("text-sm text-text", className)} {...props} />;
}

export type CardFooterProps = React.ComponentProps<"div">;
export function CardFooter({ className, ...props }: CardFooterProps) {
  return (
    <div
      className={cn("flex items-center justify-end gap-2 mt-4", className)}
      {...props}
    />
  );
}
