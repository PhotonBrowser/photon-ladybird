import { Button, type ButtonProps } from "@heroui/react";
import type { ReactNode } from "react";

interface IconButtonProps extends Omit<ButtonProps, "aria-label" | "children" | "isIconOnly"> {
    ariaLabel: string;
    children: ReactNode;
}

export function IconButton({ ariaLabel, children, className, ...props }: IconButtonProps): React.JSX.Element {
    return (
        <Button {...props} aria-label={ariaLabel} className={`photon-icon-button ${className ?? ""}`.trim()} isIconOnly>
            {children}
        </Button>
    );
}
