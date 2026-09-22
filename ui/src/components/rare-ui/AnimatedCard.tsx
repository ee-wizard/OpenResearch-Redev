import { forwardRef } from "react";
import { motion, useReducedMotion, type HTMLMotionProps } from "motion/react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "../ui/cn";

const animatedCardVariants = cva(
  "relative overflow-hidden bg-background border border-border rounded-2xl shadow-card",
  {
    variants: {
      padding: {
        none: "",
        sm: "p-3",
        default: "p-4",
        lg: "p-6",
      },
      hover: {
        none: "",
        lift: "motion-safe:transition-shadow motion-safe:hover:shadow-elevated",
        scale: "",
      },
    },
    defaultVariants: {
      padding: "default",
      hover: "lift",
    },
  },
);

export type AnimatedCardProps = HTMLMotionProps<"div"> &
  VariantProps<typeof animatedCardVariants> & {
    /** Entrance delay in seconds for staggered lists. */
    delay?: number;
  };

export const AnimatedCard = forwardRef<HTMLDivElement, AnimatedCardProps>(function AnimatedCard(
  { className, padding = "default", hover = "lift", delay = 0, children, ...props },
  ref,
) {
  const reduced = useReducedMotion();
  const hoverMotion =
    reduced || hover === "none" ? undefined : hover === "lift" ? { y: -6 } : { scale: 1.02 };

  return (
    <motion.div
      ref={ref}
      className={cn(animatedCardVariants({ padding, hover }), className)}
      initial={reduced ? false : { opacity: 0, y: 12 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{
        delay,
        type: "spring",
        stiffness: 320,
        damping: 24,
      }}
      whileHover={hoverMotion}
      whileTap={reduced ? undefined : { scale: 0.98 }}
      {...props}
    >
      {children}
    </motion.div>
  );
});
