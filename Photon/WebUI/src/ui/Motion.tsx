// SPDX-License-Identifier: GPL-3.0-only
import {
    AnimatePresence,
    motion,
    useReducedMotion,
    type HTMLMotionProps,
    type Transition,
    type Variants,
} from "motion/react";
import type { PropsWithChildren } from "react";

export const motionPresets = {
    tab: {
        variants: {
            initial: { opacity: 0, width: 0, scaleX: 0.96 },
            animate: { opacity: 1, width: "var(--photon-tab-default-width)", scaleX: 1 },
            exit: { opacity: 0, width: 0, scaleX: 0.96 },
        },
        transition: { duration: 0.18, ease: [0.2, 0, 0, 1] },
    },
    popover: {
        variants: {
            initial: { opacity: 0, scale: 0.96, y: -4 },
            animate: { opacity: 1, scale: 1, y: 0 },
            exit: { opacity: 0, scale: 0.96, y: -4 },
        },
        transition: { duration: 0.14, ease: [0.2, 0, 0, 1] },
    },
} satisfies Record<string, { variants: Variants; transition: Transition }>;

export type MotionPreset = keyof typeof motionPresets;

export function Motion({
    preset,
    children,
    ...props
}: PropsWithChildren<{ preset: MotionPreset } & Omit<HTMLMotionProps<"div">, "variants">>): React.JSX.Element {
    const shouldReduceMotion = useReducedMotion();
    const { variants, transition } = motionPresets[preset];

    return (
        <motion.div
            {...props}
            variants={shouldReduceMotion ? undefined : variants}
            transition={transition}
            initial={shouldReduceMotion ? false : "initial"}
            animate="animate"
            exit={shouldReduceMotion ? undefined : "exit"}
        >
            {children}
        </motion.div>
    );
}

export { AnimatePresence, motion };
