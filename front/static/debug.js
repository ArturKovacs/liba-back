
const logQueue = [];

/** @type {HTMLElement | null} */
let debugPanel = null;

function getOrCretateDebugPanel() {
    if (debugPanel) {
        if (!debugPanel.parentElement) {
            if (document.body) {
                document.body.appendChild(debugPanel);
            }
        } 
        return debugPanel;
    }

    debugPanel = document.createElement("div");
    if (document.body) {
        document.body.appendChild(debugPanel);
    }
    debugPanel.id = "debug-panel";
    debugPanel.style.position = "fixed";
    debugPanel.style.bottom = "0";
    debugPanel.style.left = "0";
    debugPanel.style.right = "0";
    debugPanel.style.height = "100px";
    debugPanel.style.overflowY = "auto";
    debugPanel.style.textWrap = "wrap";
    debugPanel.style.overflowWrap = "break-word";
    debugPanel.style.backgroundColor = "rgba(0, 0, 0, 0.8)";
    debugPanel.style.color = "white";
    debugPanel.style.fontFamily = "monospace";
    debugPanel.style.fontSize = "12px";
    return debugPanel;
}

/**
 * @param  {...any} args 
 */
function showDebugMessage(...args) {
    const debugPanel = getOrCretateDebugPanel();
    const logEntry = document.createElement("div");
    logEntry.style.padding = "5px";
    logEntry.style.borderBottom = "1px solid #ccc";

    logEntry.textContent = args.map(arg => {
        if (typeof arg === "object") {
            try {
                return JSON.stringify(arg);
            } catch (error) {
                return String(arg);
            }
        }
        return String(arg);
    }).join(" ");
    debugPanel.appendChild(logEntry);
    debugPanel.scrollTop = debugPanel.scrollHeight;
}

if (window.location.pathname === "/debug") {
    const originalConsoleLog = console.log;
    console.log = function (...args) {
        originalConsoleLog.apply(console, args);
        showDebugMessage(...args);
    }

    const originalConsoleWarn = console.warn;
    console.warn = function (...args) {
        originalConsoleWarn.apply(console, args);
        showDebugMessage("WARN:", ...args);
    }

    const originalConsoleError = console.error;
    console.error = function (...args) {
        originalConsoleError.apply(console, args);
        showDebugMessage("ERROR:", ...args);
    }
}
