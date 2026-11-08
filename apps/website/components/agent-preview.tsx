/**
 * A static agent session on the home page: the assistant finds a snippet, expands it with a
 * fill-in value, and gets a redacted clipboard item. Illustrative only.
 */
export function AgentPreview() {
  return (
    <figure className="agent-art feature-art">
      <figcaption className="visually-hidden">
        An example session: an AI assistant finds your welcome snippet, fills in a name, and sees
        a copied API key only as redacted.
      </figcaption>
      <div className="agent-window">
        <div className="agent-titlebar">
          <span className="agent-lights" aria-hidden="true">
            <i />
            <i />
            <i />
          </span>
          <span>claude</span>
        </div>
        <div className="agent-log">
          <p className="agent-prompt">
            <span>›</span> Reply to Maya with my usual welcome note
          </p>
          <p className="agent-call">
            <b>kudzu</b> list_snippets <span>query: “welcome”</span>
          </p>
          <p className="agent-result">
            Found <code>;welcome</code> · “Welcome note” · used 42 times
          </p>
          <p className="agent-call">
            <b>kudzu</b> expand_snippet <span>;welcome · name: “Maya”</span>
          </p>
          <div className="agent-text">
            Hi Maya, welcome aboard! Here’s everything you need for your first week…
          </div>
          <p className="agent-call">
            <b>kudzu</b> list_clipboard <span>query: “staging”</span>
          </p>
          <p className="agent-result">
            STAGING_KEY=<mark>[redacted: API key]</mark>
          </p>
          <p className="agent-reply">
            Here’s your reply to Maya, using your welcome note. I left out the staging key, since
            Kudzu keeps secrets on your Mac.
          </p>
        </div>
      </div>
    </figure>
  );
}
