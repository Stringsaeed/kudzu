import Link from "next/link";
import type { Metadata } from "next";
import { CopyBlock } from "@/components/copy-block";
import { DocsNav } from "@/components/docs-nav";
import { pageMetadata } from "@/lib/metadata";
import { site } from "@/lib/site";

const description =
  "Add dates, clipboard contents, fill-in fields, dropdowns, key presses, and nested snippets to your Kudzu shortcuts.";

export const metadata: Metadata = pageMetadata({
  title: "Template syntax: dates, forms, and variables",
  description,
  path: "/docs/templates",
});

const tags = [
  {
    tag: "{{date}}",
    does: "Today’s date, in yyyy-MM-dd format by default.",
    example: '{{date format="MMMM d, yyyy" offset="+1w"}}',
  },
  {
    tag: "{{time}}",
    does: "The current time, as 14:30 by default. Takes the same options as date.",
    example: '{{time format="h:mm a"}}',
  },
  {
    tag: "{{clipboard}}",
    does: "Whatever you last copied, as plain text. Add history=1 for the copy before that, up to 99 copies back.",
    example: "{{clipboard trim=true}}",
  },
  {
    tag: "{{selection}}",
    does: "The text you had selected when you opened quick search. Empty for a typed shortcut, because typing replaces the selection.",
    example: "“{{selection trim=true}}”",
  },
  {
    tag: "{{cursor}}",
    does: "Where your caret lands after the text is inserted.",
    example: "Hi {{cursor}},",
  },
  {
    tag: '{{field name="…"}}',
    does: "A text box that appears before inserting. Repeat a name to reuse its value. Add multiline=true for a bigger box.",
    example: '{{field name="Client" default="there"}}',
  },
  {
    tag: '{{choice name="…" options="…"}}',
    does: "A dropdown that appears before inserting. Separate options with |, and write \\| for a literal pipe.",
    example: '{{choice name="Tone" options="Warm|Direct|Brief" default="Warm"}}',
  },
  {
    tag: '{{key name="…"}}',
    does: "Presses a key at that point: tab, return, escape, space, backspace, delete, left, right, up, or down. Add count to repeat it, up to 50 times.",
    example: '{{key name="tab" count=2}}',
  },
  {
    tag: "{{delay ms=…}}",
    does: "Waits before the next step, for pages that need a moment. Up to 5 seconds each and 10 seconds in total.",
    example: "{{delay ms=300}}",
  },
  {
    tag: "{{uuid}}",
    does: "A new random ID each time. Add upper=true for capitals.",
    example: "{{uuid}}",
  },
  {
    tag: '{{random options="…"}}',
    does: "One of your options, picked at random. Separate options with |.",
    example: '{{random options="Thanks|Thank you|Much appreciated"}}',
  },
  {
    tag: '{{snippet id="…"}}',
    does: "Another snippet, included in place. Insert adds it for you, so you never type the id.",
    example: '{{snippet id="4F2A…"}}',
  },
];

export default function TemplatesPage() {
  return (
    <main id="main" className="page wrap docs-layout">
      <DocsNav current="/docs/templates" />
      <div>
        <header className="page-heading">
          <Link className="eyebrow" href="/docs">
            DOCS
          </Link>
          <h1>Template guide.</h1>
          <p>{description}</p>
        </header>
        <div className="prose">
          <p>
            In the editor, type <kbd>/</kbd> or use <strong>Insert</strong> to add a variable, then
            click its chip to configure it with a live example. You never have to write these by
            hand. Here’s the form they take, for when you want to.
          </p>
          <div className="table-scroll">
            <table>
              <thead>
                <tr>
                  <th scope="col">Tag</th>
                  <th scope="col">What it does</th>
                </tr>
              </thead>
              <tbody>
                {tags.map((item) => (
                  <tr key={item.tag}>
                    <th scope="row">
                      <code>{item.tag}</code>
                    </th>
                    <td>
                      {item.does}
                      <code className="tag-example">{item.example}</code>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          <h2>Dates and times</h2>
          <p>
            <code>format</code> takes an ICU pattern, such as <code>yyyy-MM-dd</code> or{" "}
            <code>EEEE, MMMM d</code>. <code>offset</code> moves the date: <code>+3d</code>,{" "}
            <code>-1w</code>, <code>+2m</code>, <code>+1y</code>, <code>-4h</code>, or{" "}
            <code>+30min</code>. You can combine them, as in <code>+1w2d</code>.{" "}
            <code>locale</code> sets the language, such as <code>en_US</code>.
          </p>

          <h2>Forms</h2>
          <p>
            A snippet with a <code>field</code> or <code>choice</code> shows a small panel near your
            caret with a live preview. <kbd>Tab</kbd> moves between fields, <kbd>↵</kbd> inserts, and{" "}
            <kbd>Esc</kbd> cancels and leaves your trigger as typed. Fields remember their last
            values.
          </p>

          <h2>Filling forms in other apps</h2>
          <p>
            A snippet can press keys between pieces of text, so one shortcut can fill several
            boxes on a web page. This one types a name, moves to the next box, types an email
            address, and submits:
          </p>
          <CopyBlock
            label="form-filling snippet"
            value={'Sam Rivera{{key name="tab"}}sam@example.com{{key name="return"}}'}
          />
          <ul>
            <li>
              Add <code>{"{{delay ms=300}}"}</code> where a page needs a moment before the next
              key.
            </li>
            <li>
              Backspace right after doesn’t undo a snippet that pressed keys, because its text may
              be spread across several boxes.
            </li>
            <li>
              <code>{"{{cursor}}"}</code> only works after the last key press.
            </li>
            <li>The sequence stops if you click, switch apps, or pause Kudzu.</li>
          </ul>

          <h2>Nested snippets</h2>
          <p>
            Choose another snippet from <strong>Insert</strong> to include it, so one signature can
            live inside many replies. Update it once and it changes everywhere. Nesting goes up to
            eight levels, and Kudzu handles loops safely.
          </p>

          <h2>Formatting and images</h2>
          <p>
            Choose <strong>Formatted</strong> above the editor for bold, italic, underline, links,
            lists, and images. Apps that accept rich text get the formatting, and others get plain
            text.
          </p>

          <h2>Writing literal braces</h2>
          <p>
            To insert the text <code>{"{{date}}"}</code> itself, write <code>{"\\{{date}}"}</code>.
          </p>

          <p>
            Need every detail? Read the{" "}
            <a href={site.templateSyntax}>full template syntax reference</a>.
          </p>
        </div>
      </div>
    </main>
  );
}
