import type * as React from 'react';
type IconName = 'search'|'plus'|'star'|'star-fill'|'clock'|'chart'|'folder'|'folder-plus'|'stack'|'sliders'|'cloud'|'history'|'sort'|'tray'|'info'|'chevron-down'|'chevron-right'|'chevrons-v'|'x'|'check'|'minus'|'compose'|'bug'|'sidebar'|'bolt'|'type'|'timer'|'braces'|'keyboard'|'trash'|'leaf'|'more'|'app';
export interface IconProps { name: IconName; size?: 12 | 14 | 16 | 28; label?: string; className?: string }
export declare function Icon(props: IconProps): React.ReactElement;
export interface KbdProps { children: React.ReactNode }
export declare function Kbd(props: KbdProps): React.ReactElement;
export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> { variant?: 'primary' | 'secondary' | 'ghost' | 'danger'; size?: 'md' | 'sm'; icon?: IconName; iconEnd?: IconName }
export declare function Button(props: ButtonProps): React.ReactElement;
export interface IconButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> { icon: IconName; label: string; menu?: boolean; active?: boolean; size?: 'md' | 'sm'; tone?: 'bloom' }
export declare function IconButton(props: IconButtonProps): React.ReactElement;
export interface ToolbarGroupProps { children: React.ReactNode }
export declare function ToolbarGroup(props: ToolbarGroupProps): React.ReactElement;
export interface SearchFieldProps extends React.InputHTMLAttributes<HTMLInputElement> { shortcut?: string | false }
export declare function SearchField(props: SearchFieldProps): React.ReactElement;
export interface SidebarItemProps extends React.ButtonHTMLAttributes<HTMLButtonElement> { icon?: IconName; label: React.ReactNode; count?: number; shortcut?: string; active?: boolean }
export declare function SidebarItem(props: SidebarItemProps): React.ReactElement;
export interface SidebarSectionProps { label?: string; action?: React.ReactNode; children: React.ReactNode }
export declare function SidebarSection(props: SidebarSectionProps): React.ReactElement;
export interface TriggerChipProps { children: string; removable?: boolean; onRemove?: () => void }
export declare function TriggerChip(props: TriggerChipProps): React.ReactElement;
export interface SnippetRowProps extends React.HTMLAttributes<HTMLDivElement> { name: string; triggers: string[]; preview?: string; group?: string; uses?: number; favorite?: boolean; selected?: boolean }
export declare function SnippetRow(props: SnippetRowProps): React.ReactElement;
export interface ListHeaderProps { title: React.ReactNode; subtitle?: React.ReactNode; actions?: React.ReactNode }
export declare function ListHeader(props: ListHeaderProps): React.ReactElement;
export interface FieldProps { label: React.ReactNode; aside?: React.ReactNode; hint?: React.ReactNode; htmlFor?: string; children: React.ReactNode }
export declare function Field(props: FieldProps): React.ReactElement;
export interface TextInputProps extends React.InputHTMLAttributes<HTMLInputElement> { mono?: boolean }
export declare function TextInput(props: TextInputProps): React.ReactElement;
export interface TriggerInputProps { triggers?: string[]; placeholder?: string }
export declare function TriggerInput(props: TriggerInputProps): React.ReactElement;
export interface EditorProps { value: Array<string | { tag: string }>; minHeight?: number; readOnly?: boolean }
export declare function Editor(props: EditorProps): React.ReactElement;
export interface SegmentedProps { options: Array<string | { value: string; label: React.ReactNode }>; value?: string; onChange?: (value: string) => void; label?: string }
export declare function Segmented(props: SegmentedProps): React.ReactElement;
export interface SelectProps extends React.ButtonHTMLAttributes<HTMLButtonElement> { value: React.ReactNode }
export declare function Select(props: SelectProps): React.ReactElement;
export interface ToggleProps { checked?: boolean; onChange?: (on: boolean) => void; label: string }
export declare function Toggle(props: ToggleProps): React.ReactElement;
export interface StepperProps { value: number; step?: number; min?: number; max?: number; unit?: string }
export declare function Stepper(props: StepperProps): React.ReactElement;
export interface SettingsGroupProps { title?: React.ReactNode; description?: React.ReactNode; action?: React.ReactNode; children: React.ReactNode }
export declare function SettingsGroup(props: SettingsGroupProps): React.ReactElement;
export interface SettingsRowProps { label: React.ReactNode; description?: React.ReactNode; icon?: IconName; children?: React.ReactNode }
export declare function SettingsRow(props: SettingsRowProps): React.ReactElement;
export interface StatTileProps { label: string; value: string; icon?: IconName; note?: string }
export declare function StatTile(props: StatTileProps): React.ReactElement;
export interface UsageBarProps { label: string; value: number; max: number }
export declare function UsageBar(props: UsageBarProps): React.ReactElement;
export interface MenuItem { label: string; checked?: boolean; icon?: IconName; shortcut?: string; active?: boolean; danger?: boolean }
export interface MenuProps { title?: string; items: Array<MenuItem | '-'> }
export declare function Menu(props: MenuProps): React.ReactElement;
export interface BadgeProps { tone?: 'neutral' | 'vine' | 'bloom' | 'warning' | 'danger'; children: React.ReactNode }
export declare function Badge(props: BadgeProps): React.ReactElement;
export interface ListRowProps extends React.HTMLAttributes<HTMLDivElement> { title: React.ReactNode; meta?: React.ReactNode; aside?: React.ReactNode; badge?: React.ReactNode; selected?: boolean }
export declare function ListRow(props: ListRowProps): React.ReactElement;
export interface EmptyStateProps { title: React.ReactNode; icon?: IconName; action?: React.ReactNode; children?: React.ReactNode }
export declare function EmptyState(props: EmptyStateProps): React.ReactElement;
export interface DisclosureProps { title: React.ReactNode; open?: boolean; summary?: React.ReactNode; children?: React.ReactNode }
export declare function Disclosure(props: DisclosureProps): React.ReactElement;
export interface AppShellProps { sidebar: React.ReactNode; sidebarFooter?: React.ReactNode; list: React.ReactNode; detail: React.ReactNode; style?: React.CSSProperties }
export declare function AppShell(props: AppShellProps): React.ReactElement;
declare global { interface Window { Kudzu: { Icon: typeof Icon; Kbd: typeof Kbd; Button: typeof Button; IconButton: typeof IconButton; ToolbarGroup: typeof ToolbarGroup; SearchField: typeof SearchField; SidebarItem: typeof SidebarItem; SidebarSection: typeof SidebarSection; TriggerChip: typeof TriggerChip; SnippetRow: typeof SnippetRow; ListHeader: typeof ListHeader; Field: typeof Field; TextInput: typeof TextInput; TriggerInput: typeof TriggerInput; Editor: typeof Editor; Segmented: typeof Segmented; Select: typeof Select; Toggle: typeof Toggle; Stepper: typeof Stepper; SettingsGroup: typeof SettingsGroup; SettingsRow: typeof SettingsRow; StatTile: typeof StatTile; UsageBar: typeof UsageBar; Menu: typeof Menu; Badge: typeof Badge; ListRow: typeof ListRow; EmptyState: typeof EmptyState; Disclosure: typeof Disclosure; AppShell: typeof AppShell } } }
