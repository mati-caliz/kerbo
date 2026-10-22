// +build linux darwin
// +build amd64

// Package sample ejercita la sintaxis que imprime el formateador.
package sample

import 	 (
  "strings" 
    "fmt"  
  errs "errors"
    ."math"
 _	"embed"	
			`os`

	// Un comentario antes de un import.
	"sort"
 "bytes" 	 // comentario al final
)	

import 	 "io"

const(
 	First = iota // el primero
 Second
 third    ="tercero"
 Fourth int =4	// con tipo
    longerConstantName= 0X1F
 exponent	= 	 1E3
    octal =0O17
  binary  = 0B101
 imaginary=0012i
  hexFloat=0x1P-2
	)  

var(
 	single int
    pair, other= 1, 2
  typed string="texto"
	untyped=  3.5
)

 var global=map[string]int{"a":1,"bb": 2,
 "ccc":  3}

type  (
 Point struct {	
 X,Y  float64  `json:"x"`  
 Name	string  `json:"name"`  // nombre
 	*Embedded
  fmt.Stringer
 inner 	 struct{a int}	
 }

 Embedded  struct{}
			Alias=Point
	Number interface{
			~int | ~int64|  float64
  }
    Shape interface{
 Area()float64
 Perimeter() (total float64,err 	 error)
    fmt.Stringer
	}
 Pair[K comparable, V any]struct { 	 
	Key   K 
 Value V
	}
	Ptr[T 	 any]	interface{*T }
  Empty  interface{}
)

type	List[T any][]T

func (p *Point) Area() float64 { return p.X *p.Y }

func(p Point)  String() string 	 {
 	return fmt.Sprintf("(%v, %v)", p.X, p.Y)
			}

func	Map[T,U any](items []T, transform func(T) U)[]U {
	result:=make([]U,0,len(items))
 for  _,item:=range items { 
    result= append(result, transform(item))
 }
    return  result
}

func 	 variadic(prefix string,  values	...int) (count int){ 	 
 	count = len(values)
			return

}  

  func expressions(a, b, 	 c int, s[]int, m map[string][]int) 	 int {
 x  :=a*b +  c
    y :=  (a+ b)	*c
			z :=	a+b*c-	a/b%c 
 w 	 := a<<2|b>>1  &^  c

    cond 	 :=  a > b 	 && b < c || a==c&& !(b!=c)	
 _ 	 =  s[a+1 :  b-1] 
			_  = s[1:2:3]
    _ =	s[a:] 
    _  = s[:b]
  _ = m["key"][0]
  ptr	:= &x
  *ptr= -y
    _ 	 =  ^z
			var  ch  =make(chan	int,1)
 	var recv <-chan int 	 =ch
			var send chan<-int =  ch
 	send <-  <-recv	
  _ = cond
	_ = w 
	f  := func(v	int)  int{return v * 	 2}
    _ =f(Map([]int{1,2},func(i int)int { 	 return i	})[0])
 var iface interface{}=x
 if n, ok 	 :=iface.(int);ok{
 x+= n
  }
 	_ 	 =  *(*[]byte)(nil) 	 
	_=	[]byte("texto")
			_ 	 ='r'
    _	=  `raw
string`
    return x+y + z
  }

func	statements(values []int)(result int, err error) {
 	defer  func() 	 {
  if recovered := 	 recover(); recovered!= nil{
			err= errs.New("panic")
	}
 	}()
    go 	 func() 	 {}()
 for i := 0; i <len(values);i++ {
	result+=  values[i]
	}
    for result< 	 100 {
  result*= 2
    } 	 
 for{  
 	break
  }
 	for range 3  {  
 	result--
			}
 	for  index := range values{
    result  -=  index  
 }
outer:
  for _, 	 value:=range values {
 switch 	 {
	case value >10:	
	continue outer
 	case value <0:
	break outer
 default:
	result++
 }
	}
 	switch	value:= result %3;value  {
    case 0, 1:
  result++ 
 fallthrough
 	case 2: 	 
			result--  
 }
  var shape  interface{}= result
			switch typed:=  shape.(type){
 case int,	int64:
 result =int(typed.(int))
 case	nil:
    default: 
	_=typed
 } 
 ch	:= make(chan int)
 select {
	case value	:= <-ch:
 result =value  
 	case	ch <- 1:
  default:
 	}
    if result > 0  { 	 
			result=1
 	}	else if result  <0{
	result =  -1
 }else{
	goto	done
			}
 done:
			return 	 result,nil
			}

func 	 literals() 	 []Point {
 points:=  []Point{
	{X: 	 1, Y:  2},
 {X: 	 3, Y:4,Name:"nombre"},
	}
 	matrix := [][]int{{1,2}, 	 {3,4}}
	_ =[...]string{"a","b"}
 _ = map[string]Point{
			"short":  {},
 "a much much longer key that breaks the alignment of the column":{X: 1},
 	"x": {},
    } 
 	_= matrix
 	return points
 	}

func longCall()  {
  fmt.Println("uno",
			"dos", 	 
  "tres")
    strings.NewReplacer("a","b").
  Replace("abc")
 	_= 	 bytes.NewBuffer(nil)
 	_= io.EOF
 	_ =os.Args

    _  = sort.Ints
    _=Sqrt(2)
}  

			func multiline(a, 	 b int)  bool {
	return	a>  0&&
  b	> 0
}

func multiReturn() (int,int)	{
 	return	1,
 2	
} 
