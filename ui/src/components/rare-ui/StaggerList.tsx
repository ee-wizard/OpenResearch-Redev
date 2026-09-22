import { Children, isValidElement, type ReactNode } from "react";
import { motion, useReducedMotion, type HTMLMotionProps } from "motion/react";
import { cn } from "../ui/cn";

const itemVariants = {
  hidden: { opacity: 0, y: 16 },
  visible: {
    opacity: 1,
    y: 0,
    transition: {
      type: "spring" as const,
      stiffness: 320,
      damping: 24,
    },
  },
} as const;

export type StaggerListProps = HTMLMotionProps<"div"> & {
  staggerDelay?: number;
  itemHover?: "none" | "lift" | "scale";
};

export function StaggerList({
  className,
  children,
  staggerDelay,
  itemHover = "none",
  ...props
}: StaggerListProps) {
  const reduced = useReducedMotion();

  return (
    <motion.div
      className={cn(className)}
      initial={reduced ? false : "hidden"}
      animate="visible"
      variants={{
        hidden: { opacity: 1 },
        visible: {
          opacity: 1,
          transition: {
            staggerChildren: staggerDelay ?? 0.05,
            delayChildren: 0.02,
          },
        },
      }}
      {...props}
    >
      {Children.toArray(children as ReactNode).map((child, index) => {
        if (!isValidElement(child)) return child;
        return (
          <StaggerItem key={String(child.key ?? index)} hover={itemHover}>
            {child as ReactNode}
          </StaggerItem>
        );
      })}
    </motion.div>
  );
}

export type StaggerItemProps = HTMLMotionProps<"div"> & {
  hover?: "none" | "lift" | "scale";
};

export function StaggerItem({
  className,
  hover = "none",
  children,
  ...props
}: StaggerItemProps) {
  const reduced = useReducedMotion();
  const hoverMotion =
    reduced || hover === "none"
      ? undefined
      : hover === "lift"
        ? { y: -3 }
        : { scale: 1.01 };

  return (
    <motion.div
      className={cn(className)}
      variants={itemVariants}
      whileHover={hoverMotion}
      whileTap={reduced || hover === "none" ? undefined : { scale: 0.995 }}
      transition={{ type: "spring", stiffness: 420, damping: 22 }}
      {...props}
    >
      {children}
    </motion.div>
  );
}
