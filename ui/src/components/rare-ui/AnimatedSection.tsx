import { motion, useReducedMotion, type HTMLMotionProps } from "motion/react";
import { cn } from "../ui/cn";

export type AnimatedSectionProps = HTMLMotionProps<"div"> & {
  /** Entrance delay in seconds. */
  delay?: number;
};

/** Page/section entrance: fade in while sliding up. */
export function AnimatedSection({
  className,
  delay = 0,
  children,
  ...props
}: AnimatedSectionProps) {
  const reduced = useReducedMotion();

  return (
    <motion.div
      className={cn(className)}
      initial={reduced ? false : { opacity: 0, y: 18 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{
        delay,
        type: "spring",
        stiffness: 320,
        damping: 26,
      }}
      {...props}
    >
      {children}
    </motion.div>
  );
}
