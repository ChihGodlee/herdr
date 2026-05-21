// installed by herdr
// safe to edit. this plugin only activates inside herdr-managed panes.
// HERDR_INTEGRATION_ID=opencode
// HERDR_INTEGRATION_VERSION=2

import net from "node:net";

const SOURCE = "herdr:opencode";
let reportSeq = Date.now() * 1000;

function nextReportSeq() {
  reportSeq += 1;
  return reportSeq;
}

function extractSessionId(event) {
  if (!event) return undefined;
  const props = event.properties ?? {};
  // Try common locations: properties.sessionID, properties.session.id,
  // event.sessionID, event.session.id (camelCase or snake_case).
  return (
    props.sessionID ??
    props.session_id ??
    (typeof props.session === "object" ? props.session?.id : undefined) ??
    event.sessionID ??
    event.session_id ??
    (typeof event.session === "object" ? event.session?.id : undefined)
  );
}

function reportState(action, sessionId) {
  const paneId = process.env.HERDR_PANE_ID;
  const socketPath = process.env.HERDR_SOCKET_PATH;

  if (!paneId || !socketPath) {
    return Promise.resolve();
  }

  const requestId = `${SOURCE}:${Date.now()}:${Math.floor(Math.random() * 1_000_000)
    .toString()
    .padStart(6, "0")}`;

  let params;
  if (action === "release") {
    params = {
      pane_id: paneId,
      source: SOURCE,
      agent: "opencode",
      seq: nextReportSeq(),
    };
  } else {
    params = {
      pane_id: paneId,
      source: SOURCE,
      agent: "opencode",
      state: action,
      seq: nextReportSeq(),
    };
    if (sessionId) {
      params.session_id = sessionId;
    }
  }

  const request = {
    id: requestId,
    method: action === "release" ? "pane.release_agent" : "pane.report_agent",
    params,
  };

  return new Promise((resolve) => {
    const client = net.createConnection(socketPath, () => {
      client.write(`${JSON.stringify(request)}\n`);
    });

    const finish = () => {
      client.destroy();
      resolve();
    };

    client.setTimeout(500, finish);
    client.on("data", finish);
    client.on("error", finish);
    client.on("end", finish);
    client.on("close", resolve);
  });
}

export const HerdrAgentStatePlugin = async () => {
  if (
    process.env.HERDR_ENV !== "1" ||
    !process.env.HERDR_SOCKET_PATH ||
    !process.env.HERDR_PANE_ID
  ) {
    return {};
  }

  return {
    event: async ({ event }) => {
      const type = event?.type;
      const properties = event?.properties ?? {};
      const sessionId = extractSessionId(event);

      switch (type) {
        case "permission.asked":
        case "question.asked":
          await reportState("blocked", sessionId);
          break;
        case "permission.replied": {
          const reply = properties.reply ?? properties.response;
          if (reply === "reject") {
            await reportState("idle", sessionId);
          } else if (reply === "once" || reply === "always") {
            await reportState("working", sessionId);
          }
          break;
        }
        case "question.replied":
          await reportState("working", sessionId);
          break;
        case "question.rejected":
          await reportState("idle", sessionId);
          break;
        case "session.status": {
          const status =
            typeof properties.status === "string"
              ? properties.status
              : properties.status?.type;
          if (status === "busy" || status === "retry") {
            await reportState("working", sessionId);
          } else if (status === "idle") {
            await reportState("idle", sessionId);
          }
          break;
        }
        case "session.idle":
          await reportState("idle", sessionId);
          break;
        default:
          break;
      }
    },
  };
};
