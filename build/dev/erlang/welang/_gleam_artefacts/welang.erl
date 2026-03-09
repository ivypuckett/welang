-module(welang).
-compile([no_auto_import, nowarn_unused_vars, nowarn_unused_function, nowarn_nomatch, inline]).
-define(FILEPATH, "src/welang.gleam").
-export([main/0]).

-file("src/welang.gleam", 3).
-spec main() -> nil.
main() ->
    gleam_stdlib:println(<<"Hello from welang!"/utf8>>).
