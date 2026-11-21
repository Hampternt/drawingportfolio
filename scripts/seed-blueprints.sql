-- Made-up test data for /blueprints — local dev database only. Never load it
-- into production: the visitor demo is a committed file (include_str!), not a
-- row, and nothing here is real.
--
-- Load it with python3's sqlite3 module (there may be no sqlite3 CLI):
--   python3 -c "import sqlite3; sqlite3.connect('portfolio.db').executescript(open('scripts/seed-blueprints.sql').read())"
-- or, where the CLI exists:
--   sqlite3 portfolio.db < scripts/seed-blueprints.sql
--
-- Everything belongs to the owner (the one users row with is_owner = 1). It
-- makes two projects, the ones the design canvas's boards show:
--   drawingportfolio  four plans: request-flow (the demo's request flow),
--                     blueprint-planner (one dangling wire, a folded type
--                     label), sorting-board, and scratch (empty)
--   garden-sensor     no plans: the empty-project state
-- Re-running replaces the previous seed: it deletes the owner's projects of
-- those two names and their plans first.
BEGIN;
DELETE FROM blueprint_plans WHERE project_id IN (
  SELECT id FROM blueprint_projects
  WHERE name IN ('drawingportfolio', 'garden-sensor') AND user_id = (SELECT id FROM users WHERE is_owner = 1));
DELETE FROM blueprint_projects
  WHERE name IN ('drawingportfolio', 'garden-sensor') AND user_id = (SELECT id FROM users WHERE is_owner = 1);
INSERT INTO blueprint_projects (user_id, name) SELECT id, 'drawingportfolio' FROM users WHERE is_owner = 1;
INSERT INTO blueprint_projects (user_id, name) SELECT id, 'garden-sensor' FROM users WHERE is_owner = 1;
INSERT INTO blueprint_plans (project_id, name, payload, rev)
  SELECT id, 'request-flow', '{"v":1,"root":"main","graphs":{"main":{"nodes":{"nginx":{"kind":"machine","status":"done","note":"TLS, static cache, body limit","in":{"https":"Request"},"out":{"req":"Request","static":"File"}},"router":{"kind":"machine","status":"done","note":"axum Router; ai routes to add","in":{"req":"Request"},"out":{"page":"Request","api":"Request","ai":"Request"}},"session":{"kind":"machine","status":"done","note":"AuthSession, RequireOwner","in":{"req":"Request"},"out":{"viewer":"Viewer"}},"token_gate":{"kind":"machine","status":"planned","note":"bp_ token to scope; never a cookie","in":{"bearer":"Header"},"out":{"scope":"Scope"}},"handler":{"kind":"machine","status":"done","note":"one effective viewer","in":{"req":"Request","viewer":"Viewer"},"out":{"query":"Filter","view":"Page"}},"db":{"kind":"blueprint","status":"done","graph":"db","note":"SQLite via sqlx","in":{"query":"Filter"},"out":{"rows":"Vec<Post>","plan":"PlanDoc"}},"render":{"kind":"machine","status":"done","note":"Askama, htmx fragments","in":{"rows":"Vec<Post>","view":"Page"},"out":{"html":"Html"}},"exporter":{"kind":"machine","status":"idea","note":"outline.md + JSON, no layout","in":{"plan":"PlanDoc","scope":"Scope"},"out":{"outline":"Markdown"}}},"wires":[["nginx.req","router.req"],["router.page","session.req"],["router.api","session.req"],["router.page","handler.req"],["session.viewer","handler.viewer"],["handler.query","db.query"],["handler.view","render.view"],["db.rows","render.rows"],["router.ai","token_gate.bearer"],["token_gate.scope","exporter.scope"],["db.plan","exporter.plan"],["exporter.outline","claude.context"]]},"db":{"nodes":{"pool":{"kind":"machine","status":"done","note":"one pool in AppState","in":{},"out":{"conn":"Pool"}},"posts":{"kind":"machine","status":"done","note":"posts, tags, collections","in":{"conn":"Pool"},"out":{"rows":"Vec<Post>"}},"plans":{"kind":"machine","status":"planned","note":"payload stored whole","in":{"conn":"Pool"},"out":{"plan":"PlanDoc"}}},"wires":[["pool.conn","posts.conn"],["pool.conn","plans.conn"]]}},"layout":{"main/nginx":[1,6],"main/router":[13,5],"main/session":[25,12],"main/token_gate":[25,22],"main/handler":[39,3],"main/db":[52,10],"main/render":[69,2],"main/exporter":[69,19],"db/pool":[1,2],"db/posts":[13,1],"db/plans":[13,9]}}', 1
  FROM blueprint_projects WHERE name = 'drawingportfolio' AND user_id = (SELECT id FROM users WHERE is_owner = 1);
INSERT INTO blueprint_plans (project_id, name, payload, rev)
  SELECT id, 'blueprint-planner', '{"v":1,"root":"main","graphs":{"main":{"nodes":{"editor":{"kind":"machine","status":"building","in":{"doc":"PlanDoc","rev":"Rev"},"out":{"save":"PlanDoc"},"note":"the canvas, undo, autosave"},"model":{"kind":"machine","status":"done","in":{"doc":"PlanDoc"},"out":{"checked":"PlanDoc","counts":"Summary"},"note":"blueprint-model.js, node-tested"},"save_api":{"kind":"machine","status":"building","in":{"doc":"PlanDoc","if_match":"Rev"},"out":{"rev":"Rev","counts":"Summary"},"note":"PUT with If-Match; 412 when stale"},"store":{"kind":"machine","status":"done","in":{"doc":"PlanDoc"},"out":{"doc":"PlanDoc","rev":"Rev"},"note":"payload stored whole"},"outline":{"kind":"machine","status":"done","in":{"doc":"plandoc"},"out":{"html":"Html"},"note":"Askama, beside the canvas"},"ai_read":{"kind":"machine","status":"idea","in":{"doc":"PlanDoc","token":"Token"},"out":{"md":"Markdown"},"note":"outline.md behind a token"}},"wires":[["editor.save","model.doc"],["model.checked","save_api.doc"],["store.doc","outline.doc"],["store.doc","ai_read.doc"],["store.rev","save_api.if_match"],["tokens.scope","ai_read.token"]]}},"layout":{"main/editor":[1,2],"main/model":[20,2],"main/save_api":[42,2],"main/store":[20,16],"main/outline":[64,4],"main/ai_read":[64,18]}}', 1
  FROM blueprint_projects WHERE name = 'drawingportfolio' AND user_id = (SELECT id FROM users WHERE is_owner = 1);
INSERT INTO blueprint_plans (project_id, name, payload, rev)
  SELECT id, 'sorting-board', '{"v":1,"root":"main","graphs":{"main":{"nodes":{"paste":{"kind":"machine","status":"done","in":{},"out":{"plan":"SortPlan"},"note":"the generated JSON, pasted in"},"board":{"kind":"machine","status":"done","in":{"plan":"SortPlan","rules":"VanRules"},"out":{"action":"Action"}},"log":{"kind":"machine","status":"done","in":{"action":"Action"},"out":{"board":"BoardState"},"note":"append-only, (session, seq)"},"rules":{"kind":"machine","status":"planned","in":{},"out":{"rules":"VanRules"}}},"wires":[["paste.plan","board.plan"],["rules.rules","board.rules"],["board.action","log.action"]]}},"layout":{"main/paste":[1,2],"main/rules":[1,14],"main/board":[22,6],"main/log":[44,6]}}', 1
  FROM blueprint_projects WHERE name = 'drawingportfolio' AND user_id = (SELECT id FROM users WHERE is_owner = 1);
INSERT INTO blueprint_plans (project_id, name, payload, rev)
  SELECT id, 'scratch', '{"v":1,"root":"main","graphs":{"main":{"nodes":{},"wires":[]}},"layout":{}}', 1
  FROM blueprint_projects WHERE name = 'drawingportfolio' AND user_id = (SELECT id FROM users WHERE is_owner = 1);
COMMIT;
