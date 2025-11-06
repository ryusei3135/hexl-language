#include "token.h"



static Token token_end_ptr() {
    char *end_text = (char *)malloc(5);
    strcpy(end_text, "_end");
    return (Token){end_text, TypeEnd};
}

static void assign_token_list_ptr(Token **token_list_ptr, Token assign_token, int *token_ptr_memory_count) {
    //  もし前回のトークンが"-"なら1
    static int minus_token = 0;
    int token_paren = 0;
    Token parts;

    if (!strcmp(assign_token.token, "-")) {
        minus_token = 1;
    } else if (minus_token) {
        minus_token = 0;

        if (assign_token.type == TypeNumber) {
            (*token_list_ptr)[*token_ptr_memory_count - 2].token = (char *)realloc(
                    (*token_list_ptr)[*token_ptr_memory_count - 2].token,
                    (int)strlen((*token_list_ptr)[*token_ptr_memory_count - 2].token) + 2);
            strcat((*token_list_ptr)[*token_ptr_memory_count - 2].token, assign_token.token);
            (*token_list_ptr)[*token_ptr_memory_count - 2].type = TypeNumber;
            free(assign_token.token);
            return;
        }
    }

    if (!strcmp(assign_token.token, "()")) {
        free(assign_token.token);
        assign_token.token = (char *)malloc(2);
        strcpy(assign_token.token, "(");
        assign_token.type = TypeLparen;
        token_paren = 1;

        parts.token = (char *)malloc(2);
        strcpy(parts.token, ")");
        parts.type = TypeRparen;
    }

    if (*token_ptr_memory_count > 1) {
        (*token_list_ptr) = (Token *)realloc(
                (*token_list_ptr),
                sizeof(Token)
                    * (*token_ptr_memory_count));
    }

    (*token_list_ptr)[*token_ptr_memory_count - 1] = assign_token;
    (*token_ptr_memory_count)++;

    if (token_paren) {
        assign_token_list_ptr(token_list_ptr, parts, token_ptr_memory_count);
    }
}

static Token cut_token_text(char **buffer, TokenType type) {
    int token_length = 0;

    while (is_token(*((*buffer) + token_length), type)) {
        token_length++;
    }

    char *token_text = (char *)malloc(token_length);
    strncpy(token_text, (*buffer), token_length);
    token_text[token_length] = '\0';
    *buffer += token_length;

    if (type == TypeNormal) {
        type = statement_sorting(token_text);
    }

    if (type == TypeSymbol) {
        type = change_op_symbol(token_text);
    }

    return (Token){token_text, type};
}

void free_all_token_ptr(Token *token_list_ptr) {
    int count = 0;
    while (token_list_ptr[count].type != TypeEnd) {
        free(token_list_ptr[count].token);
        count++;
    }
    free(token_list_ptr[count].token);
    free(token_list_ptr);
}

Token* make_token_list_ptr(char *buffer) {
    Token *token_list_ptr = (Token *)malloc(sizeof(Token));
    TokenType now_type;
    int token_ptr_memory_count = 1;

    int buffer_length = (int)strlen(buffer);

    while (*buffer != '\0' && *buffer != '\n') {
        if ((now_type = is_token_type(*buffer)) >= 0) {
            assign_token_list_ptr(&token_list_ptr, cut_token_text(&buffer, now_type), &token_ptr_memory_count);
            // printf("%s text\n", token_list_ptr[token_ptr_memory_count].token);
            continue;
        }

        buffer++;
    }
    assign_token_list_ptr(&token_list_ptr, token_end_ptr(), &token_ptr_memory_count);
    return token_list_ptr;
}


char* make_string_token(Token *token_list_ptr, int *pos) {
    int string_status = 0;
    char *string_token = (char *)malloc(1);

    while (token_list_ptr[*pos].type != TypeEnd) {
        if (token_list_ptr[*pos].type == TypeString) {
            if (string_status) {
                (*pos)++;
                return string_token;
            }
            string_status = 1;
            (*pos)++;
        }
        string_token = (char *)realloc(
            string_token,
            (int)strlen(token_list_ptr[*pos].token)
            + (int)strlen(string_token) + 1);
        strcat(string_token, token_list_ptr[*pos].token);
        (*pos)++;
    }
    //err
    exit(1);
}
