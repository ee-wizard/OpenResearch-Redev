import { motion, useReducedMotion, type HTMLMotionProps } from "motion/react";
import { cn } from "../ui/cn";

export type AnimatedRowProps = HTMLMotionProps<"div"> & {
  /** Entrance delay in seconds. */
  delay?: number;
  /** Hover interaction; defaults to a subtle lift. */
  hover?: "none" | "lift" | "scale";
};

/** A single list row with entrance fade + slide and a subtle hover effect. */
export function AnimatedRow({
  className,
  delay = 0,
  hover = "lift",
  children,
  ...props
}: AnimatedRowProps) {
  const reduced = useReducedMotion();
  const hoverMotion =
    reduced || hover === "none"
      ? undefined
      : hover === "lift"
        ? { y: -2 }
        : { scale: 1.005 };

  return (
    <motion.div
      className={cn(className)}
      initial={reduced ? false : { opacity: 0, y: 12 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{
        delay,
        type: "spring",
        stiffness: 360,
        damping: 24,
      }}
      whileHover={hoverMotion}
      whileTap={reduced || hover === "none" ? undefined : { scale: 0.995 }}
      {...props}
    >
      {children}
    </motion.div>
  );
}
