.PHONY: build update-snowflake

SNOWFLAKE_COMPONENTS = typography navbar sidebar inline-list footer button vertical-fill

build: env/.present
	env/bin/mkdocs build --clean

update-snowflake: env/.present
	env/bin/snowflake-css -m -c "REPLACE_COLOR_HERE" -- $(SNOWFLAKE_COMPONENTS) > theme/css/snowflake.css

env/.present: requirements.txt
	@rm -rf env
	virtualenv -p python3 env
	env/bin/pip install -r requirements.txt
	@touch env/.present
