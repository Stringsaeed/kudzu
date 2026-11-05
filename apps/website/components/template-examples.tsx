"use client";

import { useState, useSyncExternalStore } from "react";

function localDate() {
  const date = new Date();
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}

function subscribeDate(onChange: () => void) {
  const interval = setInterval(onChange, 60_000);
  return () => clearInterval(interval);
}

export function TemplateExamples() {
  const [name, setName] = useState("Alex");
  const date = useSyncExternalStore(subscribeDate, localDate, () => "YYYY-MM-DD");
  return (
    <section className="templates-section wrap" id="templates">
      <div className="section-heading">
        <div>
          <span className="feature-number">03 / A LITTLE CLEVERNESS</span>
          <h2>
            Repetition.
            <br />
            With a personal touch.
          </h2>
        </div>
        <p>
          Same starting point. The right details, every time.{" "}
          <br />
          Let your snippets do a little thinking for you.
        </p>
      </div>
      <div className="template-grid">
        <article>
          <div className="template-visual date-visual">
            <span>Today’s date, always today.</span>
            <code>
              ;today <span>→</span>{" "}
              <time dateTime={date === "YYYY-MM-DD" ? undefined : date}>{date}</time>
            </code>
            <span className="date-stamp" aria-hidden="true">
              {date.slice(-2)}
            </span>
          </div>
          <h3>Keep it current.</h3>
          <p>Drop in dates and times automatically. Tomorrow’s date? Next week’s? That too.</p>
        </article>
        <article>
          <div className="template-visual form-visual">
            <span>Make room for the human.</span>
            <div className="mini-form">
              <label htmlFor="name-example">Their name</label>
              <input
                id="name-example"
                value={name}
                onChange={(event) => setName(event.target.value)}
                maxLength={28}
                autoComplete="off"
                spellCheck={false}
              />
              <div>
                Hey <span>{name.trim() || "there"}</span>, lovely to meet you.
              </div>
            </div>
          </div>
          <h3>A template, never templated.</h3>
          <p>
            Fill in a name, choose from a dropdown, and make every reply feel like you wrote it just
            now.
          </p>
        </article>
        <article>
          <div className="template-visual nest-visual">
            <span>Good things, better together.</span>
            <div className="nested-snippet">
              <span>My go-to reply</span>
              <div>
                <span className="token">↳ Introduction</span>
                <span className="token">↳ Signature</span>
              </div>
            </div>
          </div>
          <h3>Build on your best words.</h3>
          <p>
            Nest snippets inside snippets. Update your signature once, and use it wherever it
            belongs.
          </p>
        </article>
      </div>
    </section>
  );
}
