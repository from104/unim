/*
 * UNIM XIM Test - Fields Module
 */

#ifndef FIELDS_H
#define FIELDS_H

void fields_init(void);
void fields_switch(int direction);
void fields_handle_key(XKeyEvent *event);
void fields_handle_click(XButtonEvent *event);
void fields_update_spot(void);

#endif /* FIELDS_H */
