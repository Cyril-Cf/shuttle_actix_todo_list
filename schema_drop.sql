
DROP TABLE IF EXISTS todos;

CREATE TABLE todos (
  id serial PRIMARY KEY,
  note TEXT NOT NULL,
  date_time_created TIMESTAMP NOT NULL,
  date_time_to_complete_task TIMESTAMP NOT NULL,
  owner_email TEXT NOT NULL,
  owner_password TEXT NOT NULL,
  is_started BOOLEAN NOT NULL,
  is_finished BOOLEAN NOT NULL
);