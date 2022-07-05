import * as cohoard from "https://static.witchoflight.com/~a2aaron/cohoard/0.2.0/cohoard.js";

import { Config } from "https://static.witchoflight.com/~a2aaron/cohoard/0.2.0/cohoard.js";

/**
 * Manages the `<table>` which contains the UI for editing the Config data.
 */
export class ConfigTable {
    /**
     * Construct a ConfigTable, appending a `<table>` to the given element.
     * @param {HTMLElement} element The element to mount the table to.
     * @param {Array<string>} columns The column headers.
     * @param {Array<Array<string>>} body The initial value of the body cells.
     */
    constructor(element, columns, body) {
        this.element = element;
        this.table = make_table_node(columns, body);
        element.appendChild(this.table);
    }

    /**
     * Mount a config table on the given element. Attempts to use localStorage to fill in the initial
     * table inputs.
     * @param {HTMLElement} element The element to mount the table to
     * @param {Array<string>} init_cols The initial column headers to generate.
     * May generate more columns if localStorage has more columns saved
     * @param {int} num_rows The minimum number of rows to generate
     * May generate more rows if localStore has more rows saved.
     * @returns {ConfigTable}
    */
    static mount(element, init_cols, num_rows) {
        // JS has some slightly annoying behavior with arrays. See below link.
        // https://stackoverflow.com/questions/41121982/strange-behavior-of-an-array-filled-by-array-prototype-fill
        let fallback_body = Array.from(Array(num_rows), () => Array(init_cols.length).fill(""));
        let eggbug_row = ["EGGBUG", "egg bug!", "#83254f", "https://i.imgur.com/BBaogem.png", "eggbug"]
        fallback_body[0] = eggbug_row;
        let body = localStorageOrDefault("configTableBody", fallback_body);
        let cols = localStorageOrDefault("configTableCols", init_cols);

        let table = new ConfigTable(element, cols, body);
        return table;
    }

    /** Save the table to localStorage */
    save_table() {
        let [cols, body] = array_from_table(this.table);
        localStorage.setItem("configTableBody", JSON.stringify(body));
        localStorage.setItem("configTableCols", JSON.stringify(cols));
    }

    /**
     * Get a Cohoard-usable Config object from the data represented by the table.
     * @returns {Config}
     */
    get cohoard_config() {
        return cohoard_config_from_table(this.table);
    }
}

/**
 * Generates a `<table>` of the config data
 * @param {Array<string>} cols The columns headers that will be generated
 * @param {Array<Array<string>>} body The initial table body vaues that will be generated
 * If a row of the body has less cells than cols, then empty cells will be generated.
 * If a row of the body has more cells than cols, then blank columns will be generated.
 * @returns {HTMLTableElement} The generated `table`.
 */
function make_table_node(cols, body) {
    let max_body_length = Math.max(body.map((row) => row.length));
    if (max_body_length > cols.length) {
        cols = cols.concat(Array(max_body_length - cols.length).fill(""));
    }

    let table = document.createElement("table");

    let header_row = document.createElement("tr");
    header_row.setAttribute("class", "config-row-header");
    for (let i = 0; i < cols.length; i++) {
        const col = cols[i];
        let cell = header_cell(col);

        // Update the placeholder text whenever the header cell is edited.
        if (col != "key") {
            cell.addEventListener("input", () => update_placeholders(table, i, cell.firstChild.value));
        }

        header_row.appendChild(cell);
    }

    table.appendChild(header_row);

    for (const row of body) {
        let row_node = document.createElement("tr");
        for (let i = 0; i < cols.length; i += 1) {
            let init_value = "";
            if (row[i] != undefined) {
                init_value = row[i];
            }
            row_node.appendChild(body_cell(init_value, cols[i]));
        }
        table.appendChild(row_node);
    }

    return table;
}

/**
 * Update the placeholder text of the cells in a specified column.
 * @param {HTMLTableElement} table The table to update the placeholders in
 * @param {int} col_i The index of column to update. If this is 0, then nothing happens.
 * @param {string} new_placeholder The new placeholder text to use
 */
function update_placeholders(table, col_i, new_placeholder) {
    // The first col is always the "key" column, so we can ignore it.
    if (col_i == 0) {
        return;
    }
    for (let row of table.rows) {
        // Skip the header row
        if (row.rowIndex == 0) {
            continue;
        }
        let cell = row.cells[col_i];
        assert_html_node(cell, "td");
        assert_html_node(cell.firstChild, "input");
        cell.firstChild.placeholder = new_placeholder;
    }
}

/**
 * Takes the user-displayed table and turns it into a 2D array.
 * @param {HTMLTableElement} table The table to generate the array from. 
 * This needs to be a table generated by `make_table_node`.
 * @returns {[Array<string>, Array<Array<string>>]} A tuple of the header columns and the body rows
 */
function array_from_table(table) {
    assert_html_node(table, "table");
    let cols = get_columns(table);

    let body = [];
    for (let row of table.rows) {
        // Skip the header rows
        if (row.rowIndex == 0) {
            continue;
        }
        let body_row = []
        for (let cell of row.cells) {
            assert_html_node(cell, "td");
            assert_html_node(cell.firstChild, "input");
            let cell_value = cell.firstChild.value;
            body_row.push(cell_value);
        }
        body.push(body_row);
    }
    return [cols, body];
}

/**
 * Takes the user-displayed table and turns it into an intenal Config object.
 * @param {HTMLTableElement} table The table to generate the config from. 
 * This needs to be a table generated by `make_table_node`.
 * @returns {Config} the config specified by the table.
 */
function cohoard_config_from_table(table) {
    assert_html_node(table, "table");
    let cols = get_columns(table);

    let people = [];
    row_loop: for (let row of table.rows) {
        // Skip the header rows
        if (row.rowIndex == 0) {
            continue;
        }
        let person = {}
        cell_loop: for (let cell of row.cells) {
            assert_html_node(cell, "td");
            assert_html_node(cell.firstChild, "input");
            let cell_key = cols[cell.cellIndex];
            let cell_value = cell.firstChild.value;
            if (cell_value == "") {
                if (cell_key == "key") {
                    // If the key doesn't exist, don't create a person at all
                    // (Cohoard requires the key to be set)
                    continue row_loop;
                } else {
                    // Don't set the property if the input is blank.
                    // This allows Cohoard to use a default value instead of thinking
                    // the property is set to the empty string.
                    continue cell_loop;
                }
            } else {
                person[cell_key] = cell_value;
            }
        }
        people.push(person);
    }
    let config_json = JSON.stringify({ people });
    return cohoard.load_config(config_json);
}

/**
 * Gets the values in the header row.
 * @param {HTMLElement} table 
 * @returns {Array<string>} the values of the header row.
 */
function get_columns(table) {
    assert_html_node(table, "table");
    let cols = [];
    let first_row = table.rows[0];
    for (let cell of first_row.cells) {
        assert_html_node(cell, "th");
        if (is_html_node(cell.firstChild, "input")) {
            cols.push(cell.firstChild.value);
        } else {
            cols.push("key");
        }
    }
    return cols;
}


/**
 * Return a header cell for the table.
 * @param {string} key The key for the table. if equal to `key`, then the table cell doesn't have a textinput.
 * @returns {HTMLTableCellElement} - The table cell
 */
function header_cell(key) {
    if (key == "key") {
        return h("th", {}, "key");
    } else {
        return h("th", {},
            h("input", { type: "text", placeholder: "key name", value: key })
        );
    }
}

/**
 * Return a body cell for the table.
 * @param {string} value - The initial text of the text input
 * @param {string} placeholder - The placeholder text for the text input
 * @returns {HTMLTableCellElement} - The table cell containing the text input
 */
function body_cell(value, placeholder) {
    return h("td", {},
        h("input", { type: "text", placeholder, value })
    );
}

/** Constructs HTML elements
* @param {string} tag - The tag of the HTML element
* @param {object} attrs -A dictionary of the attributes of the element
* whose keys are the attribute names and the values are the attribute values.
* Note that the "value" key (a key whose name is literally "value") is
* special--this sets the `node.value` property instead of setting an attribute.
* @param {string | HTMLElement | Array<string | HTMLElement>} [body] - The body of the HTML element.
* @returns {HTMLElement} - The constructed HTML element
* You can recursively call `h` to achieve nested objects.
* Example:
* ```javascript
* h("div", { class: "foo" }, [
*   h("h1", { id: "bar" }, "Hello!"),
*   h("p", {}, "World!"),
* ])
* ```
* This produces the following HTML
* ```html
* <div class="foo">
*    <h1 id="bar">Hello!</h1>
*    <p>World!<p>
* </div>
* ```
*/
function h(tag, attrs, body = []) {
    const element = document.createElement(tag);
    for (const [k, v] of Object.entries(attrs)) {
        // Special-case the value and have it set the actual node value
        if (k == "value") {
            element.value = v;
        } else {
            element.setAttribute(k, v);
        }
    }

    if (Array.isArray(body)) {
        element.append(...body);
    } else {
        element.append(body);
    }
    return element;
}

/**
 * @param {any} object 
 * @param {string} name The HTML element name to check for
 * @returns {bool} Returns true if `object` is an HTML element of type `name`.
 */
function is_html_node(object, name) {
    console.assert(name);
    if (object.tagName == undefined) {
        return false;
    }
    return object.tagName.toLowerCase() == name.toLowerCase();
}

/**
 * @param {any} object 
 * @param {string} name The HTML element name to check for
 */
function assert_html_node(object, name) {
    console.assert(name);
    if (!is_html_node(object, name)) {
        let tagName = object.tagName.toLowerCase();
        console.error(`object ${object} expected to be HTML node of type ${name}. Got ${tagName} instead.`);
        console.log(object.outerHTML);
    }
}

/**
 * Attempts to get an object from localStorage and parse it as JSON, falling back to a default value
 * if this fails.
 * @param {string} key the key in localStorage to look up
 * @param {T} fallback the fallback item to fall back to.
 * @returns {T} The parsed object from localStorage, or the fallback if that fails
 * @template T
 */
function localStorageOrDefault(key, fallback) {
    let obj = localStorage.getItem(key);
    if (obj == null) {
        return fallback;
    }
    try {
        return JSON.parse(obj);
    } catch (err) {
        console.warn(err);
        return fallback;
    }
}