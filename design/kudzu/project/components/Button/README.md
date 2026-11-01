# Button

Text buttons for commands: one `primary` per view at most, `secondary` for the rest, `ghost` for low-stakes or tertiary actions.

Consumer provides `children` (a verb, sentence case, with an ellipsis when it opens a picker: "Restore…"), optional `icon`, `iconEnd`, `variant` (`primary` | `secondary` | `ghost` | `danger`), `size` (`md` 30px | `sm` 24px) and the usual button props.

- Primary is a chartreuse `vine` fill with dark `on-vine` text. Never put two primaries side by side.
- Secondary sits on `surface-raised` with a `line` border; hover lays `surface-hover` over it.
- In settings rows use `size="sm"`.
- Don't use `danger` without a confirming step.
