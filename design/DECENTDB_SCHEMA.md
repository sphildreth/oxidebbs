# DecentDB Schema Sketch

This is an initial logical schema sketch, not final implementation syntax.

## users

```text
id
alias
real_name
email_optional
password_hash
security_level
is_sysop
created_at
last_login_at
total_calls
time_bank_minutes
status
```

## nodes

```text
id
node_number
status
current_user_id
current_activity
connected_at
last_activity_at
transport
```

## sessions

```text
id
node_number
user_id
transport
remote_address
started_at
ended_at
disconnect_reason
```

## message_areas

```text
id
key
name
description
kind
network_id_optional
read_security_level
post_security_level
moderated
```

## messages

```text
id
area_id
author_user_id
to_user_id_optional
subject
body
created_at
reply_to_id_optional
network_message_id_optional
visibility
```

## doors

```text
id
key
name
runner
working_dir
command
drop_file
exclusive
time_limit_minutes
enabled
```

## door_runs

```text
id
door_id
user_id
node_number
started_at
ended_at
exit_code
timed_out
disconnect_forced
bytes_in
bytes_out
```

## audit_events

```text
id
created_at
event_type
user_id_optional
node_number_optional
details
```

## system_config

```text
key
value
updated_at
```

## network_config

```text
id
key
name
kind
address
enabled
```
