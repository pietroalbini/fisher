.PHONY: build update-snowflake

SNOWFLAKE_COMPONENTS = typography navbar sidebar inline-list footer


build:
	@mkdocs build --clean

update-snowflake:
	@snowflake-css -m -c "REPLACE_COLOR_HERE" -- $(SNOWFLAKE_COMPONENTS) > theme/css/snowflake.css
